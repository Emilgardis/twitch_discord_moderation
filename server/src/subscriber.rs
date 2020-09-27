use anyhow::Context;
use async_tungstenite::{
    tokio as tokio_at,
    tungstenite::Message,
};
use futures::prelude::*;
use tokio::sync;
use tracing_futures::Instrument;
use twitch_api2::helix::Scope;
use twitch_oauth2::TwitchToken;

pub struct Subscriber {
    broadcaster_token: twitch_oauth2::UserToken,
    pub pubsub_channel: sync::broadcast::Sender<twitch_api2::pubsub::Response>,
}

impl Subscriber {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let broadcaster_token = twitch_oauth2::UserToken::from_existing(
            twitch_oauth2::client::reqwest_http_client,
            twitch_oauth2::AccessToken::new(std::env::var("BROADCASTER_OAUTH2")?),
            None,
        )
        .await
        .context("could not get broadcaster token")?;
        Ok(Subscriber {
            broadcaster_token,
            pubsub_channel: sync::broadcast::channel(16).0,
        })
    }

    pub async fn run(self) -> Result<(), anyhow::Error> {
        // Send ping every 5 minutes...
        let mut s = self.connect_and_send(twitch_api2::TWITCH_PUBSUB_URL).await?;
        
        let mut ping_timer = tokio::time::interval(std::time::Duration::new(5 * 30, 0));
        loop {
            tokio::select!(
                    _ = ping_timer.tick() => {
                        tracing::trace!("sending ping");
                        s.send(Message::Ping(vec![])).await?;
                    },

                    Some(msg) = tokio::stream::StreamExt::next(&mut s) => {
                        let span = tracing::info_span!("message received", raw_message = ?msg);
                        async {
                            let msg = match msg {
                                Err(async_tungstenite::tungstenite::Error::Protocol(e)) => {
                                    tracing::warn!("{:?}", async_tungstenite::tungstenite::Error::Protocol(e.clone()));
                                    s = self.connect_and_send(twitch_api2::TWITCH_PUBSUB_URL).await?;

                                    return Ok(())
                                },
                                _ => msg.context("when getting message")?,
                            };
                            tracing::debug!("got message");
                            match msg {
                                Message::Text(msg) => {
                                    let response = twitch_api2::pubsub::Response::parse(&msg)?;
                                    tracing::info!(message = ?response);
                                    self.pubsub_channel
                                        .send(response)
                                        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                                }
                                Message::Close(_) => {return Err(anyhow::anyhow!("twitch requested us to close the shop..."))}
                                _ => {}
                            }
                            Ok(())
                        }.instrument(span).await?;
                    },
            );
        }
    }

    pub async fn connect(
        &self,
        url: &str,
    ) -> Result<async_tungstenite::WebSocketStream<tokio_at::ConnectStream>, anyhow::Error>
    {
        tracing::debug!("connecting to {}", url);
        let (socket, _) = tokio_at::connect_async(url::Url::parse(url)?)
            .await
            .context("Can't connect")?;

        Ok(socket)
    }

    pub async fn connect_and_send(&self, url: &str) -> Result<async_tungstenite::WebSocketStream<tokio_at::ConnectStream>, anyhow::Error> {
        let mut s = self.connect(url).await?;

        let id = self
            .broadcaster_token
            .validate_token(twitch_oauth2::client::reqwest_http_client)
            .await?
            .user_id
            .context("no userid")?
            .parse()?;
        let topic = twitch_api2::pubsub::ChatModeratorActions {
            channel_id: id,
            user_id: id,
        };
        s.send(Message::text(
            twitch_api2::pubsub::TopicSubscribe::Listen {
                nonce: None,
                topics: vec![topic.into()],
                auth_token: self.broadcaster_token.token().clone(),
            }
            .to_message()
        ?))
        .await?;
        Ok(s)
    }
}
