use crate::util::Sanitize;
use tokio::sync;
use twitch_api2::{pubsub::moderation, types};
pub struct Webhook {
    webhook: discord_webhook::Webhook,
    channel_login: types::UserName,
    channel_bot_name: Option<types::DisplayName>,
}

impl Webhook {
    fn add_streamcardlink(&self, user_login: &str) -> String {
        format!(
            "[{1}](<https://www.twitch.tv/popout/{0}/viewercard/{1}?popout=>)",
            self.channel_login, user_login
        )
    }

    pub fn new(channel_login: types::UserName, opts: &crate::Opts) -> Webhook {
        Webhook {
            webhook: discord_webhook::Webhook::from_url(&opts.discord_webhook),
            channel_login,
            channel_bot_name: opts.channel_bot_name.clone().map(types::DisplayName::new),
        }
    }

    #[tracing::instrument(name = "webhook", skip(self, recv))]
    pub async fn run(
        self,
        mut recv: sync::broadcast::Receiver<twitch_api2::pubsub::Response>,
    ) -> Result<(), anyhow::Error> {
        while let Ok(msg) = recv.recv().await {
            match msg {
                twitch_api2::pubsub::Response::Response(r) => {
                    if !r.is_successful() {
                        anyhow::bail!(
                            "pubsub returned an error {}",
                            r.error.as_deref().unwrap_or("")
                        );
                    }
                    match r.nonce.as_deref() {
                        Some(crate::subscriber::MOD_NONCE) => {
                            tracing::info!("Listening to moderator actions")
                        }
                        _ => {
                            tracing::warn!(message = ?r, "Twitch responded with an unexpected message")
                        }
                    }
                }
                twitch_api2::pubsub::Response::Message { data } => match data {
                    twitch_api2::pubsub::TopicData::ChatModeratorActions { reply, .. } => {
                        tracing::info!(moderation_action = ?reply, "got mod action");
                        self.post_moderator_action(*reply).await?;
                    }
                    message => {
                        tracing::warn!("got unknown message: {:?}", message)
                    }
                },
                twitch_api2::pubsub::Response::Pong => {
                    tracing::trace!("PONG from twitch")
                }
                twitch_api2::pubsub::Response::Reconnect => {
                    tracing::error!("Twitch needs to reconnect")
                }
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn post_moderator_action(
        &self,
        action: moderation::ChatModeratorActionsReply,
    ) -> Result<(), anyhow::Error> {
        use twitch_api2::pubsub::moderation::ModerationActionCommand;
        match action {
            moderation::ChatModeratorActionsReply::ModerationAction(
                moderation::ModerationAction {
                    args,
                    created_by,
                    moderation_action,
                    target_user_id,
                    ..
                },
            ) => {
                let bot_name = self
                    .channel_bot_name
                    .clone()
                    .map(|s| s.as_str().to_lowercase());
                let mut created_by_bot = false;
                let real_created_by = match (created_by.clone(), bot_name) {
                    (created_by, Some(bot_specified_name))
                        if created_by.as_str() == bot_specified_name.to_lowercase() =>
                    {
                        match args
                            .iter()
                            .last()
                            .map_or("", |s| s.as_str())
                            .split(' ')
                            .collect::<Vec<_>>()
                            .as_slice()
                        {
                            [.., "by", user] => {
                                created_by_bot =
                                    moderation_action != ModerationActionCommand::Delete;
                                user.to_string()
                            }
                            _ => bot_specified_name,
                        }
                    }
                    (other, _) => other.to_string(),
                };

                self.webhook.send(|message| {
                    message.content(&match &moderation_action {
                        ModerationActionCommand::Delete =>  { format!("❌_Twitch Moderation_ |\n*{0}*: /delete {1} ||{2}||\n*{1}:{3}* message deleted",
                            created_by.sanitize(), // Not real created by, since delete doesn't carry that information
                            self.add_streamcardlink(args.get(0).map_or("<unknown>", |u| u)),
                            args[1..args.len().checked_sub(1).unwrap_or(1)].join(" ").sanitize(),
                            target_user_id.sanitize(),
                        )},
                        ModerationActionCommand::Timeout => format!("🔨_Twitch Moderation_ |\n*{0}*: /timeout {1}\n*{2}:{3}* has been timed out for {4}",
                            real_created_by.sanitize(),
                            args.join(" ").sanitize(),
                            self.add_streamcardlink(args.get(0).map_or("<unknown>", |u| u)),
                            target_user_id.sanitize(),
                            args.get(1).map_or(String::from("<unknown>"), |u|
                                humantime::format_duration(std::time::Duration::new(u.parse().unwrap_or(0),0)).to_string()
                            ),
                        ),
                        ModerationActionCommand::Untimeout => format!("🔨_Twitch Moderation_ |\n*{0}*: /unban {1}\n*{1}:{2}* is no longer timed out",
                            real_created_by.sanitize(),
                            self.add_streamcardlink(args.get(0).map_or("<unknown>", |u| u)),
                            target_user_id.sanitize(),
                        ),
                        ModerationActionCommand::Ban  => format!("🏝️_Twitch Moderation_ |\n*{0}*: /ban {1}\n*{2}:{3}* is now banned",
                            real_created_by.sanitize(),
                            args.join(" ").sanitize(),
                            self.add_streamcardlink(args.get(0).map_or("<unknown>", |u| u)),
                            target_user_id.sanitize(),
                        ),
                        ModerationActionCommand::Unban => format!("🏝️_Twitch Moderation_ |\n*{0}*: /unban {1}\n*{2}:{3}* is no longer banned",
                            real_created_by.sanitize(),
                            args.join(" ").sanitize(),
                            self.add_streamcardlink(args.get(0).map_or("<unknown>", |u| u)),
                            target_user_id.sanitize(),
                        ),
                        | moderation::ModerationActionCommand::ModifiedAutomodProperties
                        | moderation::ModerationActionCommand::AutomodRejected
                        | moderation::ModerationActionCommand::ApproveAutomodMessage
                        | moderation::ModerationActionCommand::DeniedAutomodMessage => format!("👀_Twitch Moderation_ |\n*{0}*: /{1} ||{2}||", created_by, moderation_action, args.join(" ")),
                        _ => format!("👀_Twitch Moderation_ |\n*{0}*: /{1} {2}", real_created_by.sanitize(), moderation_action, args.join(" ").sanitize()),

                    });
                    // .tts(false)
                    if created_by_bot {
                        message.username(&format!("{}@twitch via {}", real_created_by, self.channel_bot_name.clone().unwrap_or_else(|| types::DisplayName::from("<bot>"))))
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

            moderation::ChatModeratorActionsReply::DenyUnbanRequest(unban_request)
            | moderation::ChatModeratorActionsReply::ApproveUnbanRequest(unban_request) => {
                self.webhook
                    .send(|message| {
                        message.content(&format!(
                            "🔨_Twitch Moderation_ |\n*{0}*: /{1} {2} : {3}",
                            unban_request.created_by_login.sanitize(),
                            unban_request.moderation_action,
                            unban_request.target_user_login.sanitize(),
                            unban_request.moderator_message.sanitize()
                        ))
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e.to_string()))?;
            }
            moderation::ChatModeratorActionsReply::VipAdded(vip_added) => {
                self.webhook
                    .send(|message| {
                        message.content(&format!(
                            "👀_Twitch Moderation_ |\n*{0}*: /{1} {2}",
                            vip_added.created_by.sanitize(),
                            "vip",
                            vip_added.target_user_login.sanitize(),
                        ))
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e.to_string()))?;
            }
            moderation::ChatModeratorActionsReply::ChannelTermsAction(channel_term) => {
                let msg = match channel_term.type_ {
                    moderation::ChannelAction::AddPermittedTerm => {
                        format!(
                            "👀_Twitch Moderation_ |\n*{0}*: Added permitted term `{1}`",
                            channel_term.requester_login.sanitize(),
                            channel_term.text
                        )
                    }
                    moderation::ChannelAction::DeletePermittedTerm => {
                        format!(
                            "👀_Twitch Moderation_ |\n*{0}*: Deleted permitted term `{1}`",
                            channel_term.requester_login.sanitize(),
                            channel_term.text
                        )
                    }
                    moderation::ChannelAction::AddBlockedTerm => {
                        format!(
                            "👀_Twitch Moderation_ |\n*{0}*: Added blocked term ||`{1}`||",
                            channel_term.requester_login.sanitize(),
                            channel_term.text
                        )
                    }
                    moderation::ChannelAction::DeleteBlockedTerm => {
                        format!(
                            "👀_Twitch Moderation_ |\n*{0}*: Deleted blocked term ||`{1}`||",
                            channel_term.requester_login.sanitize(),
                            channel_term.text
                        )
                    }
                    _ => (return Ok(())),
                };
                self.webhook
                    .send(|message| message.content(&msg))
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e.to_string()))?;
            }
            moderation::ChatModeratorActionsReply::ModeratorAdded(moderator_added) => {
                self.webhook
                    .send(|message| {
                        message.content(&format!(
                            "👀_Twitch Moderation_ |\n*{0}*: Added `{1}` as moderator",
                            moderator_added.created_by.sanitize(),
                            moderator_added.target_user_login
                        ))
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e.to_string()))?;
            }
            _ => (),
        }

        Ok(())
    }
}
