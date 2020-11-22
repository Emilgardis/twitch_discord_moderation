use tokio::sync;
use twitch_api2::pubsub::moderation;
pub struct Webhook {
    webhook: discord_webhook::Webhook,
}

impl Webhook {
    pub fn new(url: &str) -> Webhook {
        Webhook {
            webhook: discord_webhook::Webhook::from_url(url),
        }
    }

    pub async fn run(
        self,
        mut recv: sync::broadcast::Receiver<twitch_api2::pubsub::Response>,
    ) -> Result<(), anyhow::Error>
    {
        while let Ok(msg) = recv.recv().await {
            match msg {
                twitch_api2::pubsub::Response::Response(r) => {
                    if !r.is_successful() {
                        anyhow::bail!("pubsub returned an error {}", r.error.unwrap());
                    }
                }
                twitch_api2::pubsub::Response::Message { data } => match data {
                    twitch_api2::pubsub::TopicData::ChannelBitsEventsV2 { .. } => {
                        todo!("bits not implemented")
                    }
                    twitch_api2::pubsub::TopicData::ChatModeratorActions { reply, .. } => {
                        self.post_moderator_action(*reply).await?;
                    }
                    twitch_api2::pubsub::TopicData::ChannelPointsChannelV1 { reply, .. } => {
                        tracing::info!(
                            reply = tracing::field::debug(&*reply),
                            "Channel points channel event!"
                        );
                    }
                    twitch_api2::pubsub::TopicData::ChannelSubscribeEventsV1 { reply, .. } => {
                        tracing::info!(
                            reply = tracing::field::debug(&reply),
                            "channel subscription event :D"
                        );
                    }
                    _ => {}
                },
            }
        }
        Ok(())
    }

    pub async fn post_moderator_action(
        &self,
        action: moderation::ChatModeratorActionsReply,
    ) -> Result<(), anyhow::Error>
    {
        use twitch_api2::pubsub::moderation::ModerationActionCommand;
        match action {
            moderation::ChatModeratorActionsReply::ModerationAction {
                args,
                created_by,
                moderation_action,
                target_user_id,
                ..
            } => {
                let bot_name = std::env::var("CHANNEL_BOT_NAME").map(|s| s.to_lowercase());
                let mut created_by_bot = false;
                let real_created_by = match created_by.clone() {
                    bot if bot_name.map_or(false, |s| s == bot) => match args
                        .iter()
                        .last()
                        .map_or("", |s| s.as_str())
                        .split(' ')
                        .collect::<Vec<_>>()
                        .as_slice()
                    {
                        [.., "by", user] => {
                            created_by_bot = moderation_action != ModerationActionCommand::Delete;
                            user.to_string()
                        }
                        _ => std::env::var("CHANNEL_BOT_NAME").unwrap(), // Checked above
                    },
                    other => other,
                };

                self.webhook.send(|message| {
                    message.content(&match &moderation_action {
                        ModerationActionCommand::Delete =>  { format!("❌_Twitch Moderation_ |\n*{0}*: /delete {1} ||{2}||\n*{1}:{3}* message deleted",
                            created_by, // Not real created by, since delete doesn't carry that information
                            args.get(0).map_or("<unknown>", |u| &u),
                            args[1..args.len().checked_sub(1).unwrap_or(1)].join(" "),
                            target_user_id,
                        )},
                        ModerationActionCommand::Timeout => format!("🔨_Twitch Moderation_ |\n*{0}*: /timeout {1}\n*{2}:{3}* has been timed out for {4}",
                            real_created_by,
                            args.join(" "),
                            args.get(0).map_or("<unknown>", |u| &u),
                            target_user_id,
                            args.get(1).map_or(String::from("<unknown>"), |u|
                                humantime::format_duration(std::time::Duration::new(u.parse().unwrap_or(0),0)).to_string()
                            ),
                        ),
                        //ModerationActionCommand::Untimeout => format!("🔨_Twitch Moderation_ |\n*{0}*: /unban {1}\n*{1}:{2}* is no longer timed out",
                        //    real_created_by,
                        //    args.get(0).map_or("<unknown>", |u| &u),
                        //    target_user_id,
                        //),
                        ModerationActionCommand::Ban  => format!("🏝️_Twitch Moderation_ |\n*{0}*: /ban {1}\n*{2}:{3}* is now banned",
                            real_created_by,
                            args.join(" "),
                            args.get(0).map_or("<unknown>", |u| &u),
                            target_user_id,
                        ),
                        ModerationActionCommand::Unban => format!("🏝️_Twitch Moderation_ |\n*{0}*: /unban {1}\n*{2}:{3}* is no longer banned",
                            real_created_by,
                            args.join(" "),
                            args.get(0).map_or("<unknown>", |u| &u),
                            target_user_id,
                        ),
                        automod if automod.to_string().starts_with("automod") || automod.to_string().starts_with("add_blocked_term  ") => format!("👀_Twitch Moderation_ |\n*{0}*: /{1} ||{2}||", created_by, moderation_action, args.join(" ")),
                        _ =>  format!("👀_Twitch Moderation_ |\n*{0}*: /{1} {2}", real_created_by, moderation_action, args.join(" ")),
                    });
                    // .tts(false)
                    if created_by_bot {
                        message.username(&format!("{}@twitch via {}", real_created_by, std::env::var("CHANNEL_BOT_NAME").unwrap_or_else(|_| String::from("<bot>"))))
                    } else {
                        message.username(&format!("{}@twitch", real_created_by))
                    }
                    // .embed(|embed| embed
                    //     .title()
                    //     .color(0xffc0cb)
                    //     .field("args", &format!("{:?}",args), true)
                    // )
                } ).await.map_err(|e| anyhow::anyhow!("{}", e.to_string()))?;
            }
            _ => {}
        }

        Ok(())
    }
}
