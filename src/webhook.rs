use crate::util::Sanitize;
use tokio::sync;
use twitch_api::{
    eventsub::channel::moderate::{self, ActionV2},
    types,
};
pub struct Webhook {
    pub webhook: discord_webhook::Webhook,
    pub channel_login: types::UserName,
    pub channel_bot_name: Option<types::DisplayName>,
}

impl Webhook {
    fn add_streamcardlink(&self, user_login: &str) -> String {
        format!(
            "[{1}](<https://www.twitch.tv/popout/{0}/viewercard/{1}?popout=>)",
            self.channel_login.sanitize(),
            user_login
        )
    }

    pub fn new(channel_login: types::UserName, opts: &crate::Opts) -> Webhook {
        Webhook {
            webhook: discord_webhook::Webhook::from_url(opts.discord_webhook.as_str()),
            channel_login,
            channel_bot_name: opts.channel_bot_name.clone().map(types::DisplayName::new),
        }
    }

    #[tracing::instrument(name = "webhook", skip(self, recv))]
    pub async fn run(
        self,
        mut recv: sync::broadcast::Receiver<crate::subscriber::Events>,
    ) -> Result<(), eyre::Report> {
        while let Ok(msg) = recv.recv().await {
            tracing::info!("Received event {:?}", msg);
            match msg {
                crate::subscriber::Events::ChannelModerateV2(p, t) => {
                    self.post_moderator_action(p.action, p.moderator_user_login, t)
                        .await?
                }
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn post_moderator_action(
        &self,
        action: ActionV2,
        moderator: types::UserName,
        timestamp: types::Timestamp,
    ) -> Result<(), eyre::Report> {
        let mut message = None;
        let done_by = format!("{}@twitch", moderator,);
        match action {
            // translation of the old commented code to more modern code and using eventsub instead of pubsub
            ActionV2::Delete(moderate::Delete {
                user_id,
                user_login,
                message_body,
                ..
            }) => {
                message = Some(format!(
                        "❌_Twitch Moderation_ |\n*{moderator}*: /delete {usercard} ||{message_body}||\n*{usercard}:{user_id}* message deleted",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                        message_body = message_body.sanitize(),
                    ));
            }
            ActionV2::Timeout(moderate::Timeout {
                user_id,
                user_login,
                expires_at,
                reason,
                ..
            }) => {
                // eventsub gives expires_at as a timestamp, so we need to calculate the duration
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /timeout {usercard}\n*{usercard}:{user_id}* has been timed out until <t:{expires}>{reason}",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                        expires = expires_at.to_utc().unix_timestamp(),
                        reason = if let Some(reason) = reason {
                            format!("\nreason: {}", reason.sanitize())
                        } else {
                            "".to_string()
                        },
                    ));
            }
            ActionV2::Untimeout(moderate::Untimeout {
                user_id,
                user_login,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /untimeout {usercard}\n*{usercard}:{user_id}* is no longer timed out",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::Ban(moderate::Ban {
                user_id,
                user_login,
                reason,
                ..
            }) => {
                message = Some(format!(
                        "🏝️_Twitch Moderation_ |\n*{moderator}*: /ban {usercard}\n*{usercard}:{user_id}* is now banned{reason}",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                        reason = if let Some(reason) = reason {
                            format!("\nreason: {}", reason.sanitize())
                        } else {
                            "".to_string()
                        },
                    ));
            }
            ActionV2::Unban(moderate::Unban {
                user_id,
                user_login,
                ..
            }) => {
                message = Some(format!(
                        "🏝️_Twitch Moderation_ |\n*{moderator}*: /unban {usercard}\n*{usercard}:{user_id}* is no longer banned",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::Followers(moderate::Followers {
                follow_duration_minutes,
                ..
            }) => {
                message = Some(format!(
                        "🔒_Twitch Moderation_ |\n*{moderator}*: /followers {follow_duration_minutes}m\nFollowers-only mode is now enabled for {follow_duration_minutes} minutes",
                    ));
            }
            ActionV2::Slow(moderate::Slow {
                wait_time_seconds, ..
            }) => {
                message = Some(format!(
                        "🔒_Twitch Moderation_ |\n*{moderator}*: /slow {wait_time_seconds}s\nSlow mode is now enabled with {wait_time_seconds} seconds",
                    ));
            }
            ActionV2::Vip(moderate::Vip {
                user_id,
                user_login,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /vip {usercard}\n*{usercard}:{user_id}* is now a VIP",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::Unvip(moderate::Unvip {
                user_id,
                user_login,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /unvip {usercard}\n*{usercard}:{user_id}* is no longer a VIP",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::Mod(moderate::Mod {
                user_id,
                user_login,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /mod {usercard}\n*{usercard}:{user_id}* is now a moderator",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::Unmod(moderate::Unmod {
                user_id,
                user_login,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /unmod {usercard}\n*{usercard}:{user_id}* is no longer a moderator",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::Raid(moderate::Raid {
                user_id,
                user_login,
                viewer_count,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /raid {usercard} {viewer_count}\n*{usercard}:{user_id}* is now being raided",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::Unraid(moderate::Unraid {
                user_id,
                user_login,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /unraid {usercard}\n*{usercard}:{user_id}* raid was canceled",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                    ));
            }
            ActionV2::ApproveUnbanRequest(moderate::UnbanRequest {
                user_id,
                user_login,
                moderator_message,
                ..
            }) => {
                message = Some(format!(
                    "🔨_Twitch Moderation_ |\n*{moderator}*: /approve {usercard} : {moderator_message}\n*{usercard}:{user_id}* unban was approved",
                    usercard = self.add_streamcardlink(user_login.as_str()),
                    moderator_message = moderator_message.sanitize(),
                ));
            }
            ActionV2::DenyUnbanRequest(moderate::UnbanRequest {
                user_id,
                user_login,
                moderator_message,
                ..
            }) => {
                message = Some(format!(
                    "🔨_Twitch Moderation_ |\n*{moderator}*: /deny {usercard} : {moderator_message}\n*{usercard}:{user_id}* unban was denied",
                    usercard = self.add_streamcardlink(user_login.as_str()),
                    moderator_message = moderator_message.sanitize(),
                ));
            }
            ActionV2::SharedChatBan(moderate::SharedChatBan(_))
            | ActionV2::SharedChatUnban(moderate::SharedChatUnban(_))
            | ActionV2::SharedChatTimeout(moderate::SharedChatTimeout(_))
            | ActionV2::SharedChatUntimeout(moderate::SharedChatUntimeout(_))
            | ActionV2::SharedChatDelete(moderate::SharedChatDelete(_)) => {
                // NOP
            },
            ActionV2::EmoteOnly => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /emoteonly\nEmote-only mode is now enabled")),
            ActionV2::EmoteOnlyOff => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /emoteonlyoff\nEmote-only mode is now disabled")),
            ActionV2::FollowersOff => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /followersoff\nFollowers-only mode is now disabled")),
            ActionV2::Uniquechat => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /uniquechat\nUnique chat is now enabled")),
            ActionV2::UniquechatOff => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /uniquechatoff\nUnique chat is now disabled")),
            ActionV2::SlowOff => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /slowoff\nSlow mode is now disabled")),
            ActionV2::Subscribers => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /subscribers\nSubscribers-only mode is now enabled")),
            ActionV2::SubscribersOff => message = Some(format!("🔒_Twitch Moderation_ |\n*{moderator}*: /subscribersoff\nSubscribers-only mode is now disabled")),
            ActionV2::AddBlockedTerm(terms)
            | ActionV2::AddPermittedTerm(terms)
            | ActionV2::RemoveBlockedTerm(terms)
            | ActionV2::RemovePermittedTerm(terms) => {
                // either add or remove
                let action = match (terms.action, terms.from_automod, terms.list) {
                    (moderate::AutomodTermAction::Add, true, moderate::AutomodTermList::Blocked) => "temp_term_add_block".to_owned(),
                    (moderate::AutomodTermAction::Add, true, moderate::AutomodTermList::Permitted) => "temp_term_add_permit".to_owned(),
                    (moderate::AutomodTermAction::Add, false, moderate::AutomodTermList::Blocked) => "term_add_block".to_owned(),
                    (moderate::AutomodTermAction::Add, false, moderate::AutomodTermList::Permitted) => "term_add_permit".to_owned(),
                    (moderate::AutomodTermAction::Remove, true, moderate::AutomodTermList::Blocked) => "temp_term_remove_block".to_owned(),
                    (moderate::AutomodTermAction::Remove, true, moderate::AutomodTermList::Permitted) => "temp_term_remove_permit".to_owned(),
                    (moderate::AutomodTermAction::Remove, false, moderate::AutomodTermList::Blocked) => "term_remove_block".to_owned(),
                    (moderate::AutomodTermAction::Remove, false, moderate::AutomodTermList::Permitted) => "term_remove_permit".to_owned(),
                    (a, from_automod, list) => format!("unknown_{a:?}_{from_automod}_{list:?}",),
                };
                message = Some(format!(
                    "🔨_Twitch Moderation_ |\n*{moderator}*: /{action} {terms}\nTerms {action}ed{temp}: {terms}",
                    action = action,
                    temp = if terms.from_automod { " temporarily" } else { "" },
                    terms = terms.terms.join(", "),
                ));

            }
            ActionV2::Warn(moderate::Warn {
                user_id,
                user_login,
                reason,
                chat_rules_cited, // Option<Vec<String>>,
                ..
            }) => {
                message = Some(format!(
                        "🔨_Twitch Moderation_ |\n*{moderator}*: /warn {usercard}\n*{usercard}:{user_id}* has been warned{chat_rules_cited}{reason}",
                        usercard = self.add_streamcardlink(user_login.as_str()),
                        chat_rules_cited = if let Some(rules) = chat_rules_cited {
                            format!(" for breaking rules: {}", rules.join(", "))
                        } else {
                            "".to_string()
                        },
                        reason = if let Some(reason) = reason {
                            format!("\nreason: {}", reason.sanitize())
                        } else {
                            "".to_string()
                        },
                    ));
            },
            _ => {
                tracing::warn!("Unknown action {:?}", action);
            }
        }
        if let Some(text) = message {
            self.webhook
                .send(|message| {
                    message.content(&text);
                    message.username(&done_by)
                })
                .await
                .map_err(|e| eyre::eyre!(e.to_string()))?;
        }
        Ok(())
    }
}
