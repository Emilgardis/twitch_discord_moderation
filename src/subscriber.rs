use anyhow::Context;
use async_tungstenite::{tokio as tokio_at, tungstenite::Message};
use futures::prelude::*;
use tokio::sync;
use tracing_futures::Instrument;
use twitch_oauth2::{TwitchToken, UserToken};

pub const MOD_NONCE: &str = "moderator";
pub struct Subscriber {
    pub(crate) access_token: twitch_oauth2::UserToken,
    pub channel_id: twitch_api2::types::UserId,
    pub channel_login: twitch_api2::types::UserName,
    pub token_id: twitch_api2::types::UserId,
    pub pubsub_channel: sync::broadcast::Sender<twitch_api2::pubsub::Response>,
}

pub async fn make_token(token: String) -> Result<UserToken, anyhow::Error> {
    UserToken::from_existing(
        twitch_oauth2::client::reqwest_http_client,
        twitch_oauth2::AccessToken::new(token),
        None,
        None,
    )
    .await
    .context("could not use access token")
    .map_err(Into::into)
}

pub async fn get_access_token(
    client: &reqwest::Client,
    opts: &crate::Opts,
) -> Result<UserToken, anyhow::Error> {
    if let Some(ref access_token) = opts.access_token {
        make_token(access_token.clone()).await
    } else if let (Some(ref oauth_service_url), Some(ref pointer)) =
        (&opts.oauth2_service_url, &opts.oauth2_service_pointer)
    {
        tracing::info!(
            "using oauth service on `{}` to get oauth token",
            oauth_service_url
        );

        let mut request = client.get(oauth_service_url);
        if let Some(ref key) = opts.oauth2_service_key {
            request = request.bearer_auth(key);
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
                    .context("when transforming oauth service response to json")?;
                make_token(
                    service_response
                        .pointer(&pointer)
                        .with_context(|| format!("could not get a field on `{}`", pointer))?
                        .as_str()
                        .context("token is not a string")?
                        .to_string(),
                )
                .await
            }
            Ok(response_error) => {
                let status = response_error.status();
                let error = response_error.text().await?;
                anyhow::bail!(
                    "oauth service returned error code: {} with body: {:?}",
                    status,
                    error
                );
            }
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("calling oauth service on `{}`", &oauth_service_url))
            }
        }
    } else {
        panic!("got empty vals for token cli group")
    }
}

impl Subscriber {
    #[tracing::instrument(skip(opts))]
    pub async fn new(opts: &crate::Opts) -> Result<Self, anyhow::Error> {
        let client = reqwest::Client::default();
        let access_token = get_access_token(&client, opts)
            .await
            .context("when getting access token")?;
        let token_id = access_token
            .validate_token(twitch_oauth2::client::reqwest_http_client)
            .await?
            .user_id
            .context("no user id found for oauth2 token, this is a bug")?;
        // if env:CHANNEL_ID or env:CHANNEL_LOGIN are not set, then assume we're using the token owner as channel
        let (channel_id, channel_login) = if let Some(ref id) = opts.channel_id {
            // use access token to fetch broadcaster login
            (
                id.clone(),
                twitch_api2::HelixClient::with_client(client.clone())
                    .get_user_from_id(id.clone(), &access_token)
                    .await
                    .context("when calling twitch api")?
                    .with_context(|| format!("there is no user id {}", &id))?
                    .login,
            )
        } else if let Some(ref login) = opts.channel_login {
            // use access token to fetch broadcaster id
            (
                twitch_api2::HelixClient::with_client(client.clone())
                    .get_user_from_login(login.clone(), &access_token)
                    .await
                    .context("when calling twitch api")?
                    .with_context(|| format!("there is no user with login name {}", &login))?
                    .id,
                login.clone(),
            )
        } else {
            // FIXME: Use the same client?
            tracing::info!("Using the same user_id as token for channel id");
            (
                token_id.clone(),
                access_token
                    .login()
                    .context("no user login attached to token")?
                    .to_string(),
            )
        };
        tracing::info!("successfully retrieved token and user info");
        Ok(Subscriber {
            access_token,
            channel_id,
            channel_login,
            token_id,
            pubsub_channel: sync::broadcast::channel(16).0,
        })
    }

    #[tracing::instrument(name = "subscriber", skip(self, opts), fields(
        self.channel_id = %self.channel_id,
        self.channel_login = %self.channel_login,
        self.token_id = %self.token_id,
    ))]
    pub async fn run(mut self, opts: &crate::Opts) -> Result<(), anyhow::Error> {
        let mut s = self
            .connect_and_send(twitch_api2::TWITCH_PUBSUB_URL)
            .await
            .context("when establishing connection")?;

        let ping_timer = tokio::time::sleep(
            std::time::Duration::new(4 * 60, 0)
                + std::time::Duration::from_millis(fastrand::u64(0..4000)),
        );
        tokio::pin!(ping_timer);
        tracing::info!("pinging every {} seconds with some jitter", 4 * 60);
        let token_timer = tokio::time::sleep(
            self.access_token
                .expires_in()
                .checked_sub(std::time::Duration::from_secs(
                    opts.oauth2_service_refresh.unwrap_or(30),
                ))
                .unwrap_or_default(),
        );
        tokio::pin!(token_timer);
        loop {
            tokio::select!(
                    _ = &mut token_timer => {
                        if opts.oauth2_service_url.is_some() {
                            tracing::info!("token is expired or will expire soon");
                            //self.access_token = get_access_token(&reqwest::Client::default(), &opts).await.context("when getting access token")?;
                            //token_timer.as_mut().reset(tokio::time::Instant::now() + self.access_token.expires_in() - std::time::Duration::from_secs(opts.oauth2_service_refresh.unwrap_or(30)));
                        } else {
                            tracing::warn!("token is expired or will expire soon");
                        }
                    },
                    _ = &mut ping_timer => {
                        tracing::trace!("sending ping");
                        s.send(Message::text(r#"{"type": "PING"}"#)).await.context("when sending ping")?;
                        ping_timer.as_mut().reset(tokio::time::Instant::now() + std::time::Duration::new(4 * 60, 0)
                        + std::time::Duration::from_millis(fastrand::u64(0..4000)));
                    },
                    Some(msg) = futures::StreamExt::next(&mut s) => {
                        let span = tracing::info_span!("message received", raw_message = ?msg);
                        async {
                            let msg = match msg {
                                Err(async_tungstenite::tungstenite::Error::Protocol(async_tungstenite::tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)) => {
                                    tracing::warn!("connection was sent an unexpected frame or was reset, reestablishing it");
                                    s = self.connect_and_send(twitch_api2::TWITCH_PUBSUB_URL).await.context("when reestablishing connection")?;

                                    return Ok(())
                                },
                                _ => msg.context("when getting message")?,
                            };
                            tracing::debug!("got message");
                            match msg {
                                Message::Text(msg) => {
                                    let response = twitch_api2::pubsub::Response::parse(&msg).context("when parsing pubsub response text")?;
                                    if let twitch_api2::pubsub::Response::Reconnect = response {
                                        s = self.connect_and_send(twitch_api2::TWITCH_PUBSUB_URL).await?;
                                    }
                                    tracing::debug!(message = ?response);
                                    if let twitch_api2::pubsub::Response::Response(ref _r) = response {
                                        // TODO handle bad auth
                                    }
                                    self.pubsub_channel
                                        .send(response)?;
                                }
                                Message::Close(_) => {return Err(anyhow::anyhow!("twitch requested us to close the shop..."))}
                                Message::Ping(..) | Message::Pong(..) => {}
                                Message::Binary(v) => {
                                    tracing::debug!("got unknown binary message {:2x?}", v);
                                }
                            }
                            Ok(())
                        }.instrument(span).await?;
                    },
            );
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn connect(
        &self,
        url: &str,
    ) -> Result<async_tungstenite::WebSocketStream<tokio_at::ConnectStream>, anyhow::Error> {
        tracing::info!("connecting to twitch");
        let config = async_tungstenite::tungstenite::protocol::WebSocketConfig {
            max_send_queue: None,
            max_message_size: Some(64 << 20), // 64 MiB
            max_frame_size: Some(16 << 20),   // 16 MiB
            accept_unmasked_frames: false,
        };
        let (socket, _) = tokio_at::connect_async_with_config(url::Url::parse(url)?, Some(config))
            .await
            .context("Can't connect")?;

        Ok(socket)
    }

    #[tracing::instrument(skip(self))]
    pub async fn connect_and_send(
        &self,
        url: &str,
    ) -> Result<async_tungstenite::WebSocketStream<tokio_at::ConnectStream>, anyhow::Error> {
        use twitch_api2::pubsub::Topic as _;
        let mut s = self
            .connect(url)
            .await
            .context("when connecting to twitch")?;
        let topic = twitch_api2::pubsub::moderation::ChatModeratorActions {
            channel_id: self
                .channel_id
                .parse()
                .context("could not parse the channel user id as an integer")?,
            user_id: self
                .token_id
                .parse()
                .context("could not parse the user id of the token as an integer")?,
        }
        .into_topic();
        // if scopes doesn't contain required scope, then bail
        if !<twitch_api2::pubsub::moderation::ChatModeratorActions as twitch_api2::pubsub::Topic>::SCOPE.iter().all(|s| self.access_token.scopes().contains(&s)) {
            tracing::info!("token has scopes: {:?}", self.access_token.scopes());
            anyhow::bail!("access token does not have valid scopes, required scope(s): {:?}", <twitch_api2::pubsub::moderation::ChatModeratorActions as twitch_api2::pubsub::Topic>::SCOPE.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        }
        tracing::info!("sending pubsub subscription topics to listen to");
        s.send(Message::text(twitch_api2::pubsub::listen_command(
            &[topic],
            self.access_token.token().clone().secret().as_str(),
            Some(MOD_NONCE),
        )?))
        .await?;

        // let topic = twitch_api2::pubsub::ChannelPointsChannelV1 { channel_id: id };
        // s.send(Message::text(
        //     twitch_api2::pubsub::TopicSubscribe::Listen {
        //         nonce: Some("points".to_string()),
        //         topics: vec![topic.into()],
        //         auth_token: self.broadcaster_token.token().clone(),
        //     }
        //     .to_command()?,
        // ))
        // .await?;

        // let topic = twitch_api2::pubsub::ChannelSubscribeEventsV1 { channel_id: id };
        // s.send(Message::text(
        //     twitch_api2::pubsub::TopicSubscribe::Listen {
        //         nonce: Some("subscribe".to_string()),
        //         topics: vec![topic.into()],
        //         auth_token: self.broadcaster_token.token().clone(),
        //     }
        //     .to_command()?,
        // ))
        // .await?;

        Ok(s)
    }
}
