#[cfg(test)]
pub mod ci;
pub mod subscriber;
pub mod util;
pub mod webhook;

use clap::{builder::ArgPredicate, ArgGroup, Parser};
use eyre::WrapErr;

#[derive(Parser, Debug)]
#[clap(about, version, long_version = &**util::LONG_VERSION,
    group = ArgGroup::new("token").multiple(false).required(false),
    group = ArgGroup::new("service").multiple(true).requires("oauth2_service_url"),
    group = ArgGroup::new("dcf_oauth").multiple(true).requires("dcf_oauth_client_id"),
    group = ArgGroup::new("channel").multiple(true).required(false),
)]
pub struct Opts {
    /// URL to discord webhook.
    #[clap(long, env, hide_env = true, value_parser = url::Url::parse)]
    pub discord_webhook: url::Url,
    /// OAuth2 Access token
    #[clap(long, env, hide_env = true, group = "token",
        value_parser = is_token, required_unless_present_any = ["service", "dcf_oauth"]
    )]
    pub access_token: Option<Secret>,
    /// Name of channel to monitor. If left out, defaults to owner of access token.
    #[clap(long, env, hide_env = true, group = "channel")]
    pub channel_login: Option<String>,
    /// User ID of channel to monitor. If left out, defaults to owner of access token.
    #[clap(long, env, hide_env = true, group = "channel")]
    pub channel_id: Option<String>,
    /// URL to service that provides OAuth2 token. Called on start and whenever the token needs to be refreshed.
    ///
    /// This application does not do any refreshing of tokens.
    #[clap(long, env, hide_env = true, group = "service",
        value_parser = url::Url::parse, required_unless_present_any = ["token", "dcf_oauth"]
    )]
    pub oauth2_service_url: Option<url::Url>,
    /// Bearer key for authorizing on the OAuth2 service url.
    #[clap(long, env, hide_env = true, group = "service")]
    pub oauth2_service_key: Option<Secret>,
    /// Grab token by pointer. See https://tools.ietf.org/html/rfc6901
    #[clap(
        long,
        env,
        hide_env = true,
        group = "service",
        default_value_if("oauth2_service_url", ArgPredicate::IsPresent, Some("/access_token"))
    )]
    pub oauth2_service_pointer: Option<String>,
    /// Grab a new token from the OAuth2 service this many seconds before it actually expires. Default is 30 seconds
    #[clap(
        long,
        env,
        hide_env = true,
        group = "service",
        default_value_if("oauth2_service_url", ArgPredicate::IsPresent, Some("30"))
    )]
    pub oauth2_service_refresh: Option<u64>,
    /// Client id to get a token. Stores the token data in the path specified by `--dcf-secret` (client id and optional secret is not stored)
    #[clap(long, env, hide_env = true, group = "dcf_oauth", required_unless_present_any = ["access_token", "service"])]
    pub dcf_oauth_client_id: Option<twitch_api::twitch_oauth2::ClientId>,
    /// Client secret to get a token. Only needed for confidential applications.
    #[clap(long, env, hide_env = true, group = "dcf_oauth")]
    pub dcf_oauth_client_secret: Option<twitch_api::twitch_oauth2::ClientSecret>,
    /// Path for storing DCF oauth.
    #[clap(
        long,
        env,
        hide_env = true,
        group = "dcf_oauth",
        default_value = "./.dcf_secret"
    )]
    pub dcf_secret_path: Option<std::path::PathBuf>,
    ///
    /// Name of channel bot.
    #[clap(long, env, hide_env = true)]
    pub channel_bot_name: Option<String>,
}

pub fn is_token(s: &str) -> eyre::Result<Secret> {
    if s.starts_with("oauth:") {
        eyre::bail!("token should not have `oauth:` as a prefix")
    }
    if s.len() != 30 {
        eyre::bail!("token needs to be 30 characters long")
    }
    Ok(Secret(s.to_owned()))
}

#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    fn secret(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for Secret {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[secret]")
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv().with_context(|| "couldn't load .env file"); //ignore error
    let _ = util::build_logger();

    let opts = Opts::parse();
    tracing::info!(
        "App started!\n{}",
        Opts::try_parse_from(["app", "--version"])
            .unwrap_err()
            .to_string()
    );

    match run(&opts).await {
        Ok(_) => {}
        Err(err) => {
            tracing::error!(Error = %err, "Could not handle message");
            for err in <eyre::Report>::chain(&err).skip(1) {
                tracing::error!(Error = %err, "Caused by");
            }
        }
    }
}

pub async fn run(opts: &Opts) -> eyre::Result<()> {
    let subscriber = subscriber::Subscriber::new(opts)
        .await
        .context("when constructing subscriber")?;
    let recv = subscriber.channel.subscribe();
    let webhook = webhook::Webhook::new(subscriber.channel_login.clone(), opts);
    tracing::debug!("entering main block");
    tokio::select!(
    r = subscriber.run(opts) => {
        tracing::warn!(message = "subscriber exited early", result = ?r);
        if r.is_err() {
            r.with_context(|| "subscriber returned with error to panic on")?
        } else {
            eyre::bail!("subscriber returned early when it should not have")
        }
    },
    r = webhook.run(recv) => {
        tracing::warn!(message = "webhook exited early", result = ?r);
        if r.is_err() {
            r.with_context(|| "webhook returned with error to panic on")?
        } else {
            eyre::bail!("webhook returned early when it should not have")
        }
    });
    Ok(())
}
