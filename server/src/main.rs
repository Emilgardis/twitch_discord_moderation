pub mod subscriber;
pub mod util;
pub mod webhook;
use anyhow::Context;
use warp::Filter;

#[tokio::main]
async fn main() {
    let _ = dotenv::dotenv().with_context(|| "couldn't load .env file"); //ignore error
    let _ = util::build_logger("logfile_warn", "logfile_trace");
    match run().await {
        Ok(_) => {},
        Err(err) => {
            tracing::error!(Error = %err, "Could not handle message");
                    for err in <anyhow::Error>::chain(&err).skip(1) {
                        tracing::error!(Error = %err, "Caused by");
                    }
        }
    }
}

pub async fn run() -> anyhow::Result<()> {
    tracing::info!("App started!");

    let route = warp::any()
        .and(warp::body::bytes())
        .map(|bytes| {
            tracing::info!(body = ?bytes);
            warp::reply()
        })
        .with(warp::trace(|info| {
            // Create a span using tracing macros
            tracing::info_span!(
                "request",
                method = %info.method(),
                path = %info.path(),
            )
        }));
    let subscriber = subscriber::Subscriber::new().await?;
    let recv = subscriber.pubsub_channel.subscribe();
    let server = warp::serve(route).run(([0, 0, 0, 0], 8080));
    let webhook = webhook::Webhook::new(&std::env::var("DISCORD_WEBHOOK").context("couldn't get webhook env")?);
    tokio::select!(
        _ = server => {
            return Err(anyhow::anyhow!("server returned early..."))
        },
        r = subscriber.run() => {
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