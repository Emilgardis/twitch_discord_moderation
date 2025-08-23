use eyre::WrapErr;
use futures::TryStreamExt;
use std::sync::Arc;
use tokio::sync;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite;
use tracing_futures::Instrument;
use twitch_api::twitch_oauth2::{self, TwitchToken, UserToken};

use twitch_api::{
    eventsub::{
        self,
        event::websocket::{EventsubWebsocketData, ReconnectPayload, SessionData, WelcomePayload},
        Event,
    },
    types::{self},
    HelixClient,
};
pub const MOD_NONCE: &str = "moderator";
pub struct Subscriber {
    pub(crate) access_token: twitch_oauth2::UserToken,
    pub channel_id: twitch_api::types::UserId,
    pub channel_login: twitch_api::types::UserName,
    pub token_id: twitch_api::types::UserId,
    pub channel: sync::broadcast::Sender<Events>,
    pub client: reqwest::Client,
}

pub async fn make_token(
    client: &impl twitch_oauth2::client::Client,
    token: impl Into<twitch_oauth2::AccessToken>,
) -> Result<UserToken, eyre::Report> {
    UserToken::from_existing(client, token.into(), None, None)
        .await
        .context("could not use access token")
}

pub async fn get_dcf_token(
    client: &reqwest::Client,
    webhook: &discord_webhook::Webhook,
    scopes: Vec<twitch_oauth2::Scope>,
    client_id: twitch_oauth2::ClientId,
    client_secret: Option<twitch_oauth2::ClientSecret>,
    secret_path: std::path::PathBuf,
) -> Result<UserToken, eyre::Report> {
    // four things can happen.
    // 1. the file doesn't exist, we ask for dcf then store token and refresh.
    // 2. the file exists, but the token is expired (or about to expire), we refresh the token and store.
    // 3. the file exists and the token is still valid, we use the token.
    // 4. the file exists, but the token or data is invalid (e.g empty or corrupted), we ask for dcf then store token and refresh.

    // UserToken does not implement serde::Deserialize.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct StoredUserToken {
        access_token: twitch_oauth2::AccessToken,
        refresh_token: twitch_oauth2::RefreshToken,
    }

    let (access_token, refresh_token) = if let Ok(file) = std::fs::File::open(&secret_path) {
        if let Ok(StoredUserToken {
            access_token,
            refresh_token,
        }) = serde_json::from_reader(file)
        {
            (access_token, refresh_token)
        } else {
            // file is not correct
            let token = do_dcf_flow(client, webhook, client_id.clone(), scopes.clone()).await?;
            (token.access_token, token.refresh_token.unwrap())
        }
    } else {
        // file doesn't exist
        let token = do_dcf_flow(client, webhook, client_id.clone(), scopes.clone()).await?;
        (token.access_token, token.refresh_token.unwrap())
    };

    // validate, refresh if needed, store
    let mut token = match UserToken::from_existing_or_refresh_token(
        client,
        access_token,
        refresh_token.clone(),
        client_id.clone(),
        client_secret.clone(),
    )
    .await
    {
        Ok(token) => token,
        Err(e) => {
            tracing::warn!("could not use stored token, trying new dcf: {}", e);
            do_dcf_flow(client, webhook, client_id.clone(), scopes.clone()).await?
        }
    };

    if token.expires_in() < std::time::Duration::from_secs(60) {
        token.refresh_token(client).await?;
    }
    let validator = scopes
        .iter()
        .cloned()
        .map(|s| s.to_validator())
        .collect::<Vec<_>>();
    let validator =
        twitch_oauth2::Validator::All(twitch_oauth2::scopes::validator::Sized(validator.into()));
    if let Some(missing) = validator.missing(token.scopes()) {
        tracing::warn!(%missing, "missing scopes, trying new dcf");
        token = do_dcf_flow(client, webhook, client_id, scopes).await?;
    }
    let file = std::fs::File::create(&secret_path)?;
    serde_json::to_writer(
        file,
        &StoredUserToken {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.as_ref().unwrap().clone(),
        },
    )?;
    Ok(token)
}

pub async fn do_dcf_flow(
    client: &reqwest::Client,
    webhook: &discord_webhook::Webhook,
    client_id: twitch_oauth2::ClientId,
    scopes: Vec<twitch_oauth2::Scope>,
) -> Result<UserToken, eyre::Report> {
    let mut builder = twitch_oauth2::DeviceUserTokenBuilder::new(client_id, scopes);
    let response = builder.start(client).await?;
    let url = &response.verification_uri;
    let code = &response.user_code;
    println!("Please visit {} and enter the code: {}", url, code);
    tracing::info!("waiting for user to enter code at {}", url);
    webhook
        .send(|m| {
            m.content(&format!(
                "Please visit <{}> and enter the code: `{}` to authenticate `twitch_discord_moderation` with twitch!",
                url, code
            )).username("twitch_moderation")
        })
        .await
        .map_err(|e| eyre::eyre!("{e}"))?;
    tracing::info!("sent discord webhook");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    let token = builder.wait_for_code(client, tokio::time::sleep).await?;
    webhook
        .send(|m| {
            m.content("Successfully authenticated with twitch!")
                .username("twitch_moderation")
        })
        .await
        .map_err(|e| eyre::eyre!("{e}"))?;
    Ok(token)
}

pub async fn get_access_token(
    client: &reqwest::Client,
    opts: &crate::Opts,
) -> Result<UserToken, eyre::Report> {
    if let Some(ref access_token) = opts.access_token {
        make_token(client, access_token.secret().to_string()).await
    } else if let (Some(ref oauth_service_url), Some(ref pointer)) =
        (&opts.oauth2_service_url, &opts.oauth2_service_pointer)
    {
        tracing::info!(
            "using oauth service on `{}` to get oauth token",
            oauth_service_url
        );

        let mut request = client.get(oauth_service_url.clone());
        if let Some(ref key) = opts.oauth2_service_key {
            request = request.bearer_auth(key.secret());
        }
        let request = request.build()?;
        tracing::debug!("request: {:?}", request);

        match client.execute(request).await {
            Ok(response)
                if !(response.status().is_client_error()
                    || response.status().is_server_error()) =>
            {
                let service_response: serde_json::Value = response
                    .json()
                    .await
                    .context("could not transform oauth service response to json")?;
                make_token(
                    client,
                    service_response
                        .pointer(pointer)
                        .ok_or_else(|| eyre::eyre!("could not get a field on `{}`", pointer))?
                        .as_str()
                        .ok_or_else(|| eyre::eyre!("token is not a string"))?
                        .to_string(),
                )
                .await
            }
            Ok(response_error) => {
                let status = response_error.status();
                let error = response_error.text().await?;
                eyre::bail!(
                    "oauth service returned error code: {} with body: {:?}",
                    status,
                    error
                );
            }
            Err(e) => {
                Err(e).with_context(|| format!("calling oauth service on `{}`", &oauth_service_url))
            }
        }
    } else if let (Some(id), secret, Some(path)) = (
        &opts.dcf_oauth_client_id,
        &opts.dcf_oauth_client_secret,
        &opts.dcf_secret_path,
    ) {
        let webhook = discord_webhook::Webhook::from_url(opts.discord_webhook.as_str());
        get_dcf_token(
            client,
            &webhook,
            vec![
                twitch_oauth2::Scope::ModeratorReadBlockedTerms,
                twitch_oauth2::Scope::ModeratorReadChatSettings,
                twitch_oauth2::Scope::ModeratorReadUnbanRequests,
                twitch_oauth2::Scope::ModeratorReadBannedUsers,
                twitch_oauth2::Scope::ModeratorReadChatMessages,
                twitch_oauth2::Scope::ModeratorReadModerators,
                twitch_oauth2::Scope::ModeratorReadVips,
                twitch_oauth2::Scope::ModeratorReadWarnings,
            ],
            id.clone(),
            secret.clone(),
            path.clone(),
        )
        .await
    } else {
        panic!("got empty vals for token cli group: {:?}", opts)
    }
}

impl Subscriber {
    #[tracing::instrument(skip(opts))]
    pub async fn new(opts: &crate::Opts) -> Result<Self, eyre::Report> {
        use twitch_api::client::ClientDefault;
        let product = format!("twitch_discord_moderation/{} (https://github.com/Emilgardis/twitch_discord_moderation)", env!("CARGO_PKG_VERSION"));
        let client = reqwest::Client::default_client_with_name(Some(product.try_into()?))?;
        let access_token = get_access_token(&client, opts)
            .await
            .context("could not get access token")?;
        let token_user_id = access_token
            .validate_token(&client)
            .await?
            .user_id
            .ok_or_else(|| eyre::eyre!("no user id found for oauth2 token, this is a bug"))?;
        // if env:CHANNEL_ID or env:CHANNEL_LOGIN are not set, then assume we're using the token owner as channel
        let (channel_id, channel_login) = if let Some(ref id) = opts.channel_id {
            // use access token to fetch broadcaster login
            (
                id.clone().into(),
                twitch_api::HelixClient::with_client(client.clone())
                    .get_user_from_id(id, &access_token)
                    .await
                    .wrap_err("could not get user from id")?
                    .ok_or_else(|| eyre::eyre!("there is no user id {}", &id))?
                    .login,
            )
        } else if let Some(ref login) = opts.channel_login {
            // use access token to fetch broadcaster id
            (
                twitch_api::HelixClient::with_client(client.clone())
                    .get_user_from_login(login, &access_token)
                    .await
                    .wrap_err("could not get user from login")?
                    .ok_or_else(|| eyre::eyre!("there is no user with login name {}", &login))?
                    .id,
                login.clone().into(),
            )
        } else {
            // FIXME: Use the same client?
            tracing::info!("Using the same user_id as token for channel id");
            (
                token_user_id.clone(),
                access_token
                    .login()
                    .ok_or_else(|| eyre::eyre!("no user login attached to token"))?
                    .into(),
            )
        };
        tracing::info!("successfully retrieved token and user info");
        Ok(Subscriber {
            access_token,
            channel_id,
            channel_login,
            token_id: token_user_id,
            channel: sync::broadcast::channel(16).0,
            client: client.clone(),
        })
    }

    #[tracing::instrument(name = "subscriber", skip(self, opts), fields(
        self.channel_id = %self.channel_id,
        self.channel_login = %self.channel_login,
        self.token_id = %self.token_id,
    ))]
    pub async fn run(&self, opts: &crate::Opts) -> Result<(), eyre::Report> {
        let client = twitch_api::HelixClient::with_client(self.client.clone());

        let websocket = WebsocketClient {
            session_id: None,
            token: Arc::new(Mutex::new(self.access_token.clone())),
            client,
            connect_url: twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL.clone(),
            keepalive_timeout_seconds: 60,
            chats: vec![self.channel_id.clone()],
        };

        websocket
            .run(
                |event, timestamp| async {
                    let Some(event) = Events::new(event, timestamp) else {
                        return Ok(());
                    };
                    self.channel
                        .send(event)
                        .map_err(|_| eyre::eyre!("could not send event"))?;
                    Ok(())
                },
                opts,
            )
            .await?;
        Ok(())
    }
}

pub struct WebsocketClient {
    /// The session id of the websocket connection
    pub session_id: Option<String>,
    /// The token used to authenticate with the Twitch API
    pub token: Arc<Mutex<UserToken>>,
    /// The client used to make requests to the Twitch API
    pub client: HelixClient<'static, reqwest::Client>,
    /// The url to use for websocket
    pub connect_url: url::Url,
    /// Chats to connect to.
    pub chats: Vec<twitch_api::types::UserId>,
    keepalive_timeout_seconds: i64,
}

impl WebsocketClient {
    /// Connect to the websocket and return the stream
    async fn connect(
        &self,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        eyre::Error,
    > {
        tracing::info!("connecting to twitch");
        let config = tungstenite::protocol::WebSocketConfig::default();
        let (socket, _) =
            tokio_tungstenite::connect_async_with_config(&self.connect_url, Some(config), false)
                .await
                .wrap_err("can't connect")?;

        Ok(socket)
    }

    async fn reconnect(
        &mut self,
        opts: &crate::Opts,
        stream: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Result<(), eyre::Report> {
        {
            let mut token = self.token.lock().await;
            if token.expires_in() < std::time::Duration::from_secs(60) {
                *token = get_access_token(&self.client.clone_client(), opts).await?;
            }
        }
        *stream = self
            .connect()
            .await
            .context("could not reestablish connection")?;
        Ok(())
    }
    /// Run the websocket subscriber
    #[tracing::instrument(name = "subscriber", skip_all, fields())]
    pub async fn run<Fut>(
        mut self,
        mut event_fn: impl FnMut(Event, types::Timestamp) -> Fut,
        opts: &crate::Opts,
    ) -> Result<(), eyre::Report>
    where
        Fut: std::future::Future<Output = Result<(), eyre::Report>>,
    {
        // Establish the stream
        let mut s = self
            .connect()
            .await
            .context("connection could not be estasblished")?;

        // Loop over the stream, processing messages as they come in.
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(self.keepalive_timeout_seconds as u64),
                futures::StreamExt::next(&mut s),
            )
            .await
            {
                Err(_) => {
                    tracing::warn!(
                        "connection has not responded in {}s, reconnecting",
                        self.keepalive_timeout_seconds
                    );
                    self.reconnect(opts, &mut s).await?;
                }
                Ok(None) => {
                    tracing::warn!("connection has ended unexpectedly, reconnecting",);
                    self.reconnect(opts, &mut s).await?;
                }
                Ok(Some(msg)) => {
                    let span = tracing::debug_span!("message received", raw_message = ?msg);
                    let msg = match msg {
                        Err(tungstenite::Error::Protocol(
                            tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                        )) => {
                            tracing::warn!("connection was sent an unexpected frame or was reset, reestablishing it");
                            self.reconnect(opts, &mut s).await?;
                            continue;
                        }
                        _ => msg.context("unexpected error message")?,
                    };
                    self.process_message(msg, &mut event_fn)
                        .instrument(span)
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Process a message from the websocket
    async fn process_message<Fut>(
        &mut self,
        msg: tungstenite::Message,
        event_fn: &mut impl FnMut(Event, types::Timestamp) -> Fut,
    ) -> Result<(), eyre::Report>
    where
        Fut: std::future::Future<Output = Result<(), eyre::Report>>,
    {
        match msg {
            tungstenite::Message::Text(s) => {
                // Parse the message into a [twitch_api::eventsub::EventsubWebsocketData]
                match Event::parse_websocket(&s)? {
                    EventsubWebsocketData::Welcome {
                        payload: WelcomePayload { session },
                        ..
                    }
                    | EventsubWebsocketData::Reconnect {
                        payload: ReconnectPayload { session },
                        ..
                    } => {
                        tracing::info!("got welcome message");
                        self.process_welcome_message(session).await?;
                        Ok(())
                    }
                    EventsubWebsocketData::Notification { metadata, payload } => {
                        event_fn(payload, metadata.message_timestamp.into_owned()).await?;
                        Ok(())
                    }
                    re @ EventsubWebsocketData::Revocation { .. } => {
                        eyre::bail!("got revocation event: {re:?}")
                    }
                    EventsubWebsocketData::Keepalive {
                        metadata: _,
                        payload: _,
                    } => Ok(()),
                    _ => Ok(()),
                }
            }
            tungstenite::Message::Close(_) => todo!(),
            _ => Ok(()),
        }
    }

    async fn process_welcome_message(&mut self, data: SessionData<'_>) -> Result<(), eyre::Report> {
        tracing::info!("connected to twitch chat");
        self.session_id = Some(data.id.to_string());
        if let Some(url) = data.reconnect_url {
            self.connect_url = url.parse()?;
        }
        if let Some(kt) = data.keepalive_timeout_seconds {
            self.keepalive_timeout_seconds = kt;
        }
        let token = self.token.lock().await;
        let transport = eventsub::Transport::websocket(data.id.clone());
        for broadcaster_id in &self.chats {
            let token_user_id = token.user_id().unwrap().to_owned();
            let subs: Vec<_> = self
                .client
                .get_eventsub_subscriptions(Some(eventsub::Status::Enabled), None, None, &*token)
                .map_ok(|r| {
                    futures::stream::iter(
                        r.subscriptions
                            .into_iter()
                            .filter(|s| {
                                s.transport
                                    .as_websocket()
                                    .is_some_and(|t| t.session_id == data.id)
                            })
                            .map(Ok::<_, eyre::Report>),
                    )
                })
                .try_flatten()
                .try_collect()
                .await?;
            if !subs.is_empty() {
                continue;
            }
            // if you update the scopes needed, make sure to update do_dcf_flow() as well
            let moderate = eventsub::channel::ChannelModerateV2::new(
                broadcaster_id.clone(),
                token_user_id.clone(),
            );
            self.client
                .create_eventsub_subscription(moderate, transport.clone(), &*token)
                .await?;
            // let automod_update = eventsub::automod::AutomodTermsUpdateV1::new(
            //     broadcaster_id.clone(),
            //     token_user_id.clone(),
            // );
            // self.client
            //     .create_eventsub_subscription(automod_update, transport.clone(), &*token)
            //     .await?;
            // let automod = eventsub::automod::AutomodMessageHoldV2::new(
            //     broadcaster_id.clone(),
            //     token_user_id.clone(),
            // );
            // self.client
            //     .create_eventsub_subscription(automod, transport.clone(), &*token)
            //     .await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Events {
    // AutomodTermsUpdateV1(
    //     <eventsub::automod::AutomodTermsUpdateV1 as eventsub::EventSubscription>::Payload,
    //     types::Timestamp,
    // ),
    // AutomodMessageHoldV2(
    //     <eventsub::automod::AutomodMessageHoldV2 as eventsub::EventSubscription>::Payload,
    //     types::Timestamp,
    // ),
    ChannelModerateV2(
        <eventsub::channel::ChannelModerateV2 as eventsub::EventSubscription>::Payload,
        types::Timestamp,
    ),
}

impl Events {
    pub fn new(event: Event, timestamp: types::Timestamp) -> Option<Self> {
        let event = match event {
            // Event::AutomodTermsUpdateV1(eventsub::Payload {
            //     message: eventsub::Message::Notification(p),
            //     ..
            // }) => Events::AutomodTermsUpdateV1(p, timestamp),
            // Event::AutomodMessageHoldV2(eventsub::Payload {
            //     message: eventsub::Message::Notification(p),
            //     ..
            // }) => Events::AutomodMessageHoldV2(p, timestamp),
            Event::ChannelModerateV2(eventsub::Payload {
                message: eventsub::Message::Notification(p),
                ..
            }) => Events::ChannelModerateV2(p, timestamp),
            _ => return None,
        };
        Some(event)
    }
}
