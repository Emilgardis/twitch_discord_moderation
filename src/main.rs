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
    /// Report unrecoverable errors to the discord webhook instead of making the program exit.
    #[clap(long, env, hide_env = true)]
    pub discord_error_report: bool,
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
async fn main() -> eyre::Result<()> {
    use twitch_api::client::ClientDefault;
    let _ = dotenvy::dotenv().with_context(|| "couldn't load .env file"); //ignore error
    let _ = util::build_logger();

    let opts = Opts::parse();
    tracing::info!(
        "App started!\n{}",
        Opts::try_parse_from(["app", "--version"])
            .unwrap_err()
            .to_string()
    );
    #[allow(unused_assignments)]
    let (mut times, mut when, mut error, mut old_err) =
        (0, std::time::Instant::now(), String::new(), String::new());
    let product = format!(
        "twitch_discord_moderation/{} (https://github.com/Emilgardis/twitch_discord_moderation)",
        env!("CARGO_PKG_VERSION")
    );
    let client = reqwest::Client::default_client_with_name(Some(product.try_into()?))?;

    let err = loop {
        match run(&client, &opts).await {
            Ok(_) => {}
            Err(err) => {
                error = "".to_string();
                for err in <eyre::Report>::chain(&err) {
                    error.push_str(&format!("> {err}\n"));
                }
                if when.elapsed() > std::time::Duration::from_secs(5 * 60) {
                    times = 0;
                }
                if times == 0 {
                    old_err = error.clone();
                }
                times += 1;
                if when.elapsed() < std::time::Duration::from_secs(2) {
                    break err;
                }
                if times >= 10 {
                    break err;
                } else if times == 1 {
                    // don't sleep on the first error
                } else {
                    let backoff = 2u64.pow(times as u32).min(30);
                    tracing::warn!(
                        "Error occurred, sleeping for {backoff} seconds. Error: {error}"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
                    when = std::time::Instant::now();
                }
                tracing::error!("An error occurred.");
                for err in <eyre::Report>::chain(&err) {
                    tracing::error!(Error = %err);
                }
            }
        }
    };
    tracing::error!("An error occurred.");
    for err in <eyre::Report>::chain(&err) {
        tracing::error!(Error = %err);
    }
    if opts.discord_error_report {
        let first_error = if error != old_err && !old_err.is_empty() {
            format!("Error 1:\n```\n{old_err}\n```\nError 2:\n")
        } else {
            "".to_string()
        };
        let many = if times > 0 {
            format!(" (x{})", times + 1)
        } else {
            "".to_string()
        };
        let http = serenity::http::HttpBuilder::without_token()
            .client(client.clone())
            .build();
        let webhook =
            serenity::model::webhook::Webhook::from_url(&http, opts.discord_webhook.as_str())
                .await?;
        let message = serenity::all::ExecuteWebhook::new()
            .username("twitch_moderation")
            .content(format!(
                "The bot crashed{many}. The bot has stopped\n{first_error}```\n{error}```"
            ));
        let e = webhook.execute(&http, false, message).await;
        // message was sent ok. stall the program to prevent reporting again
        if e.is_ok() {
            tracing::info!(
                "Error report sent to discord. Stalling the program to prevent it from exiting."
            );
            futures::future::pending::<()>().await;
            unreachable!();
        } else {
            tracing::error!(
                "Error report failed to send to discord. Error: {}",
                e.unwrap_err()
            );
            // some sleeping so that we don't hit the resources again immediately if the program gets restarted
            // and the error is still there
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }
    return Err(err);
}

pub async fn run(client: &reqwest::Client, opts: &Opts) -> eyre::Result<()> {
    let subscriber = subscriber::Subscriber::new(client, opts)
        .await
        .context("could not construct subscriber")?;
    let webhook = webhook::Webhook::new(client, subscriber.channel_login.clone(), opts).await?;
    let recv = subscriber.channel.subscribe();
    tracing::debug!("entering main block");
    tokio::select!(
    r = subscriber.run(opts) => {
        tracing::warn!(message = "subscriber exited early", result = ?r);
        if r.is_err() {
            r.with_context(|| "subscriber error")?
        } else {
            eyre::bail!("subscriber returned early when it should not have")
        }
    },
    r = webhook.run(recv) => {
        tracing::warn!(message = "webhook exited early", result = ?r);
        if r.is_err() {
            r.with_context(|| "webhook error")?
        } else {
            eyre::bail!("webhook returned early when it should not have")
        }
    });
    Ok(())
}
