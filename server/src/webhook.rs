use tokio::sync;
use twitch_api2::pubsub::moderation;
pub struct Webhook {
    webhook: discord_webhook::Webhook,
}

impl Webhook {
    pub fn new(url: &str) -> Webhook {
        Webhook {
            webhook: discord_webhook::Webhook::from_url(url)
        }
    }
    pub async fn run(self, mut recv: sync::broadcast::Receiver<twitch_api2::pubsub::Response>) -> Result<(), anyhow::Error> {
        while let Ok(msg) = recv.recv().await {
            match msg {
                twitch_api2::pubsub::Response::Response(r) => {if !r.is_successful() {
                    anyhow::bail!("pubsub returned an error {}", r.error.unwrap());
                }}
                twitch_api2::pubsub::Response::Message {data } => {
                    match data {
                        twitch_api2::pubsub::TopicData::ChannelBitsEventsV2 { .. } => {
                            todo!("bits not implemented")
                        }
                        twitch_api2::pubsub::TopicData::ChatModeratorActions { reply, .. } => {
                            self.post_moderator_action(reply).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub async fn post_moderator_action(&self, action: moderation::ChatModeratorActionsReply) -> Result<(), anyhow::Error> {
        match action {
            moderation::ChatModeratorActionsReply::ModerationAction { args, created_by, moderation_action, target_user_id, .. } => {
                let real_created_by = match created_by.as_str() {
                    "sessisbot" => args.iter().last().map_or("sessisbot", |s| s.as_str()),
                    other => other,
                };
                self.webhook.send(|message| {
                    message.content(&match moderation_action.as_str() {
                        "delete" =>  format!("❌_Twitch Moderation_ |\n*{0}*: /delete {1} ||{2}||\n*{1}:{3}* message deleted",
                            real_created_by,
                            args.get(0).map_or("<unknown>", |u| &u),
                            args[1..args.len().checked_sub(1).unwrap_or(1)].join(" "),
                            target_user_id,
                        ),
                        "timeout" => format!("🔨_Twitch Moderation_ |\n*{0}*: /timeout {1}\n*{2}:{3}* has been timed out for {4}",
                            real_created_by,
                            args.join(" "),
                            args.get(0).map_or("<unknown>", |u| &u),
                            target_user_id,
                            args.get(1).map_or(String::from("<unknown>"), |u|
                                humantime::format_duration(std::time::Duration::new(u.parse().unwrap_or(0),0)).to_string()
                            ),
                        ),
                        "untimeout" => format!("🔨_Twitch Moderation_ |\n*{0}*: /unban {1}\n*{1}:{2}* is no longer timed out",
                            real_created_by,
                            args.get(0).map_or("<unknown>", |u| &u),
                            target_user_id,
                        ),
                        "ban"  => format!("🏝️_Twitch Moderation_ |\n*{0}*: /ban {1}\n*{2}:{3}* is now banned",
                            real_created_by,
                            args.join(" "),
                            args.get(0).map_or("<unknown>", |u| &u),
                            target_user_id,
                        ),
                        "unban"  => format!("🏝️_Twitch Moderation_ |\n*{0}*: /unban {1}\n*{2}:{3}* is no longer banned",
                            real_created_by,
                            args.join(" "),
                            args.get(0).map_or("<unknown>", |u| &u),
                            target_user_id,
                        ),
                        _ =>  format!("👀_Twitch Moderation_ |\n*{0}*: /{1} {2}", real_created_by, moderation_action.as_str(), args.join(" ")),
                        
                    });
                    // .tts(false)
                    if real_created_by != created_by {
                        message.username(&format!("{}@twitch via SessisBot", real_created_by))
                    } else {
                        message.username(&format!("{}@twitch", created_by))
                    }
                    // .embed(|embed| embed
                    //     .title()
                    //     .color(0xffc0cb)
                    //     .field("args", &format!("{:?}",args), true)
                    // )
                } ).await.map_err(|e| anyhow::anyhow!("{}", e.to_string()))?;
            }
            moderation::ChatModeratorActionsReply::ModeratorAdded { .. } => {}
        }
        
        Ok(())
    }
}