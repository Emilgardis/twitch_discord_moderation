pub mod subscriber;
pub mod util;
pub mod webhook;
use anyhow::Context;

#[tokio::main]
async fn main() {
    let _ = dotenv::dotenv().with_context(|| "couldn't load .env file"); //ignore error
    let _ = util::build_logger("logfile_warn", "logfile_trace");
    match run().await {
        Ok(_) => {}
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
    let subscriber = subscriber::Subscriber::new()
        .await
        .context("when constructing subscriber")?;
    let recv = subscriber.pubsub_channel.subscribe();
    let webhook = webhook::Webhook::new(
        &std::env::var("DISCORD_WEBHOOK").context("couldn't get webhook env")?,
    );
    tokio::select!(
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
