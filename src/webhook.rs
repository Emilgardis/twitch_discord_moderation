use crate::util::Sanitize;
use tokio::sync;
use twitch_api::{
    eventsub::channel::moderate::{self, ActionV2},
    types,
};
pub struct Webhook {
    pub webhook: serenity::model::webhook::Webhook,
    pub channel_login: types::UserName,
    discord_http: serenity::http::Http,
}

impl Webhook {
    fn add_streamcardlink(&self, user_login: &str) -> String {
        format!(
            "[{1}](<https://www.twitch.tv/popout/{0}/viewercard/{1}?popout=>)",
            self.channel_login.sanitize(),
            user_login
        )
    }

    pub async fn new(
        client: &reqwest::Client,
        channel_login: types::UserName,
        opts: &crate::Opts,
    ) -> Result<Self, eyre::Report> {
        let http = serenity::http::HttpBuilder::without_token()
            .client(client.clone())
            .build();
        Ok(Self {
            webhook: serenity::model::webhook::Webhook::from_url(
                &http,
                opts.discord_webhook.as_str(),
            )
            .await?,
            channel_login,
            discord_http: http,
        })
    }

    #[tracing::instrument(name = "webhook", skip(self, recv))]
    pub async fn run(
        &self,
        mut recv: sync::broadcast::Receiver<crate::subscriber::Events>,
    ) -> Result<(), eyre::Report> {
        while let Ok(msg) = recv.recv().await {
            tracing::info!("Received event {:?}", msg);
            match msg {
                crate::subscriber::Events::ChannelModerateV2(p, _t) => {
                    self.post_moderator_action(p.action, p.moderator_user_login)
                        .await?
                }
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn format_actionv2(
        &self,
        action: ActionV2,
        moderator: types::UserName,
    ) -> Option<String> {
        fn header_(emoji: &str, moderator: &types::UserName) -> String {
            format!("{emoji} _Twitch Moderation_ |\n*{moderator}*: ")
        }
        fn reason_(reason: Option<&String>) -> String {
            if let Some(reason) = reason {
                format!("\nreason: {}", reason.sanitize())
            } else {
                "".to_string()
            }
        }

        match action {
            ActionV2::Delete(moderate::Delete {
                user_id,
                user_login,
                message_body,
                ..
            }) => {
                Some(format!(
                "{header}/delete {usercard} ||{message_body}||\n*{usercard}:{user_id}* message deleted",
                header = header_("🗑️", &moderator),
                usercard = self.add_streamcardlink(user_login.as_str()),
                message_body = message_body.sanitize(),
            ))
            }
            ActionV2::Timeout(moderate::Timeout {
                user_id,
                user_login,
                expires_at,
                reason,
                ..
            }) => {
                Some(format!(
                "{header}/timeout {usercard}\n*{usercard}:{user_id}* has been timed out until <t:{expires}>{reason}",
                header = header_("⏲️", &moderator),
                usercard = self.add_streamcardlink(user_login.as_str()),
                expires = expires_at.to_utc().unix_timestamp(),
                reason = reason_(reason.as_ref()),
            ))
            }
            ActionV2::Untimeout(moderate::Untimeout {
                user_id,
                user_login,
                ..
            }) => {
                Some(format!(
                    "{header}/untimeout {usercard}\n*{usercard}:{user_id}* is no longer timed out",
                    header = header_("⏲️", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                ))
            }
            ActionV2::Ban(moderate::Ban {
                user_id,
                user_login,
                reason,
                ..
            }) => {
                Some(format!(
                    "{header}/ban {usercard}\n*{usercard}:{user_id}* is now banned{reason}",
                    header = header_("🏝️", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                    reason = reason_(reason.as_ref()),
                ))
            }
            ActionV2::Unban(moderate::Unban {
                user_id,
                user_login,
                ..
            }) => {
                Some(format!(
                    "{header}/unban {usercard}\n*{usercard}:{user_id}* is no longer banned",
                    header = header_("🏝️", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                ))
            }
            ActionV2::Followers(moderate::Followers {
                follow_duration_minutes,
                ..
            }) => {
                Some(format!(
                "{header}/followers {follow_duration_minutes}m\nFollowers-only mode is now enabled for {follow_duration_minutes} minutes",
                header = header_("🔒", &moderator),
            ))
            }
            ActionV2::Slow(moderate::Slow {
                wait_time_seconds, ..
            }) => {
                Some(format!(
                "{header}/slow {wait_time_seconds}s\nSlow mode is now enabled with {wait_time_seconds} seconds",
                header = header_("🔒", &moderator),
            ))
            }
            ActionV2::Vip(moderate::Vip {
                user_id,
                user_login,
                ..
            }) => {
                Some(format!(
                    "{header}/vip {usercard}\n*{usercard}:{user_id}* is now a VIP",
                    header = header_("⭐", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                ))
            }
            ActionV2::Unvip(moderate::Unvip {
                user_id,
                user_login,
                ..
            }) => {
                Some(format!(
                    "{header}/unvip {usercard}\n*{usercard}:{user_id}* is no longer a VIP",
                    header = header_("⭐", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                ))
            }
            ActionV2::Mod(moderate::Mod {
                user_id,
                user_login,
                ..
            }) => {
                Some(format!(
                    "{header}/mod {usercard}\n*{usercard}:{user_id}* is now a moderator",
                    header = header_("🛡️", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                ))
            }
            ActionV2::Unmod(moderate::Unmod {
                user_id,
                user_login,
                ..
            }) => {
                Some(format!(
                    "{header}/unmod {usercard}\n*{usercard}:{user_id}* is no longer a moderator",
                    header = header_("🛡️", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                ))
            }
            ActionV2::Raid(moderate::Raid {
                user_id,
                user_login,
                viewer_count,
                ..
            }) => {
                Some(format!(
                "{header}/raid {usercard} {viewer_count}\n*{usercard}:{user_id}* is now being raided",
                header = header_("🚀", &moderator),
                usercard = self.add_streamcardlink(user_login.as_str()),
            ))
            }
            ActionV2::Unraid(moderate::Unraid {
                user_id,
                user_login,
                ..
            }) => {
                Some(format!(
                    "{header}/unraid {usercard}\n*{usercard}:{user_id}* raid was canceled",
                    header = header_("🚀", &moderator),
                    usercard = self.add_streamcardlink(user_login.as_str()),
                ))
            }
            ActionV2::ApproveUnbanRequest(moderate::UnbanRequest {
                user_id,
                user_login,
                moderator_message,
                ..
            }) => {
                Some(format!(
                "{header}/approve {usercard} : {moderator_message}\n*{usercard}:{user_id}* unban was approved",
                header = header_("📨", &moderator),
                usercard = self.add_streamcardlink(user_login.as_str()),
                moderator_message = moderator_message.sanitize(),
            ))
            }
            ActionV2::DenyUnbanRequest(moderate::UnbanRequest {
                user_id,
                user_login,
                moderator_message,
                ..
            }) => {
                Some(format!(
                "{header}/deny {usercard} : {moderator_message}\n*{usercard}:{user_id}* unban was denied",
                header = header_("📨", &moderator),
                usercard = self.add_streamcardlink(user_login.as_str()),
                moderator_message = moderator_message.sanitize(),
            ))
            }
            ActionV2::SharedChatBan(moderate::SharedChatBan(_))
            | ActionV2::SharedChatUnban(moderate::SharedChatUnban(_))
            | ActionV2::SharedChatTimeout(moderate::SharedChatTimeout(_))
            | ActionV2::SharedChatUntimeout(moderate::SharedChatUntimeout(_))
            | ActionV2::SharedChatDelete(moderate::SharedChatDelete(_)) => {
                None
            }
            ActionV2::EmoteOnly => {
                Some(format!(
                    "{}/emoteonly\nEmote-only mode is now enabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::EmoteOnlyOff => {
                Some(format!(
                    "{}/emoteonlyoff\nEmote-only mode is now disabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::FollowersOff => {
                Some(format!(
                    "{}/followersoff\nFollowers-only mode is now disabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::Uniquechat => {
                Some(format!(
                    "{}/uniquechat\nUnique chat is now enabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::UniquechatOff => {
                Some(format!(
                    "{}/uniquechatoff\nUnique chat is now disabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::SlowOff => {
                Some(format!(
                    "{}/slowoff\nSlow mode is now disabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::Subscribers => {
                Some(format!(
                    "{}/subscribers\nSubscribers-only mode is now enabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::SubscribersOff => {
                Some(format!(
                    "{}/subscribersoff\nSubscribers-only mode is now disabled",
                    header_("🔒", &moderator)
                ))            }
            ActionV2::AddBlockedTerm(terms)
            | ActionV2::AddPermittedTerm(terms)
            | ActionV2::RemoveBlockedTerm(terms)
            | ActionV2::RemovePermittedTerm(terms) => {
                use moderate::AutomodTermAction::{Add, Remove};
                use moderate::AutomodTermList::{Blocked, Permitted};
                let action = match (terms.action, terms.from_automod, terms.list) {
                    (Add, true, Blocked) => "temp_term_add_block".to_owned(),
                    (Add, true, Permitted) => "temp_term_add_permit".to_owned(),
                    (Add, false, Blocked) => "term_add_block".to_owned(),
                    (Add, false, Permitted) => "term_add_permit".to_owned(),
                    (Remove, true, Blocked) => "temp_term_remove_block".to_owned(),
                    (Remove, true, Permitted) => "temp_term_remove_permit".to_owned(),
                    (Remove, false, Blocked) => "term_remove_block".to_owned(),
                    (Remove, false, Permitted) => "term_remove_permit".to_owned(),
                    (a, from_automod, list) => format!("unknown_{a:?}_{from_automod}_{list:?}",),
                };
                Some(format!(
                    "{header}/{action} {terms}\nTerms {action}ed{temp}: {terms}",
                    header = header_("📋", &moderator),
                    action = action,
                    temp = if terms.from_automod {
                        " temporarily"
                    } else {
                        ""
                    },
                    terms = terms.terms.join(", "),
                ))
            }
            ActionV2::Warn(moderate::Warn {
                user_id,
                user_login,
                reason,
                chat_rules_cited,
                ..
            }) => {
                Some(format!(
                "{header}/warn {usercard}\n*{usercard}:{user_id}* has been warned{chat_rules_cited}{reason}",
                header = header_("⚠️", &moderator),
                usercard = self.add_streamcardlink(user_login.as_str()),
                chat_rules_cited = if let Some(rules) = chat_rules_cited {
                    format!(" for breaking rules: {}", rules.join(", "))
                } else {
                    "".to_string()
                },
                reason = reason_(reason.as_ref()),
            ))
            }
            _ => {
                tracing::warn!("Unknown action {:?}", action);
                None
            }
        }
    }
    pub async fn post_moderator_action(
        &self,
        action: ActionV2,
        moderator: types::UserName,
    ) -> Result<(), eyre::Report> {
        let done_by = format!("{}@twitch", moderator,);
        let message = self.format_actionv2(action, moderator);
        if let Some(text) = message.await {
            let builder = serenity::all::ExecuteWebhook::new()
                .content(&text)
                .username(&done_by);
            self.webhook
                .execute(&self.discord_http, false, builder)
                .await?;
        }
        Ok(())
    }
}
