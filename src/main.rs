pub mod subscriber;
pub mod util;
pub mod webhook;
use anyhow::Context;
use clap::{ArgGroup, ArgSettings, Clap};

#[derive(Clap, Debug)]
#[clap(about, version, long_version = concat!(env!("VERGEN_BUILD_SEMVER"),"-", env!("GIT_SHA"),"", "\nrustc: ", env!("VERGEN_RUSTC_SEMVER"), " ", env!("VERGEN_RUSTC_COMMIT_HASH"), "\nbuild timestamp: ", env!("VERGEN_BUILD_TIMESTAMP")),
    group = ArgGroup::new("token").multiple(false).required(false), 
    group = ArgGroup::new("service").multiple(true).requires("oauth2-service-url"), 
    group = ArgGroup::new("channel").multiple(true).required(false), 
)]
pub struct Opts {
    /// URL to discord webhook.
    #[clap(long, env, setting = ArgSettings::HideEnvValues, validator = url::Url::parse)]
    pub discord_webhook: String,
    /// OAuth2 Access token
    #[clap(long, env, setting = ArgSettings::HideEnvValues, group = "token",
        validator = is_token, required_unless_present = "service"
    )]
    pub access_token: Option<Secret>,
    /// Name of channel to monitor. If left out, defaults to owner of access token.
    #[clap(long, env, setting = ArgSettings::HideEnvValues, group = "channel")]
    pub channel_login: Option<String>,
    /// User ID of channel to monitor. If left out, defaults to owner of access token.
    #[clap(long, env, setting = ArgSettings::HideEnvValues, group = "channel")]
    pub channel_id: Option<String>,
    /// URL to service that provides OAuth2 token. Called on start and whenever the token needs to be refreshed.
    ///
    /// This application does not do any refreshing of tokens.
    #[clap(long, env, setting = ArgSettings::HideEnvValues, group = "token", 
        validator = url::Url::parse, required_unless_present = "token"
    )]
    pub oauth2_service_url: Option<String>,
    /// Bearer key for authorizing on the OAuth2 service url.
    #[clap(long, env, setting = ArgSettings::HideEnvValues, group = "service")]
    pub oauth2_service_key: Option<Secret>,
    /// Grab token by pointer. See https://tools.ietf.org/html/rfc6901
    #[clap(long, env, setting = ArgSettings::HideEnvValues, group = "service", default_value_if("oauth2-service-url", None, Some("/access_token")))]
    pub oauth2_service_pointer: Option<String>,
    /// Grab a new token from the OAuth2 service this many seconds before it actually expires. Default is 30 seconds
    #[clap(long, env, setting = ArgSettings::HideEnvValues, group = "service", default_value_if("oauth2-service-url", None, Some("30")))]
    pub oauth2_service_refresh: Option<u64>,
    /// Name of channel bot.
    #[clap(long, env, setting = ArgSettings::HideEnvValues)]
    pub channel_bot_name: Option<String>,
}

pub fn is_token(s: &str) -> anyhow::Result<()> {
    if s.starts_with("oauth:") {
        anyhow::bail!("token should not have `oauth:` as a prefix")
    }
    if s.len() != 30 {
        anyhow::bail!("token needs to be 30 characters long")
    }
    Ok(())
}

#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    fn secret(&self) -> &str { &self.0 }
}

impl std::str::FromStr for Secret {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self(s.to_string())) }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "[secret]") }
}

#[tokio::main]
async fn main() {
    let _ = dotenv::dotenv().with_context(|| "couldn't load .env file"); //ignore error
    let _ = util::build_logger();

    let opts = Opts::parse();
    tracing::info!(
        "App started!\n{}",
        Opts::try_parse_from(&["app", "--version"])
            .unwrap_err()
            .to_string()
    );

    match run(&opts).await {
        Ok(_) => {}
        Err(err) => {
            tracing::error!(Error = %err, "Could not handle message");
            for err in <anyhow::Error>::chain(&err).skip(1) {
                tracing::error!(Error = %err, "Caused by");
            }
        }
    }
}

pub async fn run(opts: &Opts) -> anyhow::Result<()> {
    let subscriber = subscriber::Subscriber::new(opts)
        .await
        .context("when constructing subscriber")?;
    let recv = subscriber.pubsub_channel.subscribe();
    let webhook = webhook::Webhook::new(subscriber.channel_login.to_string(), &opts);
    tracing::debug!("entering main block");
    tokio::select!(
    r = subscriber.run(&opts) => {
        tracing::warn!(message = "subscriber exited early", result = ?r);
        if r.is_err() {
            r.with_context(|| "subscriber returned with error to panic on")?
        } else {
            anyhow::bail!("subscriber returned early when it should not have")
        }
    },
    r = webhook.run(recv) => {
        tracing::warn!(message = "webhook exited early", result = ?r);
        if r.is_err() {
            r.with_context(|| "webhook returned with error to panic on")?
        } else {
            anyhow::bail!("webhook returned early when it should not have")
        }
    });
    Ok(())
}
