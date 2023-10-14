use color_eyre::{eyre::eyre, eyre::Context, eyre::Result};
use dotenvy::dotenv;
use tokio::runtime::Builder;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::{filter::LevelFilter, fmt::time::UtcTime};

#[allow(unused_imports)]
use tracing_subscriber::prelude::*;

fn main() -> Result<()> {
    color_eyre::install().expect("Installing color_eyre to not fail.");
    dotenv().wrap_err_with(|| eyre!("Initializing .env failed!"))?;
    FmtSubscriber::builder()
        .with_timer(UtcTime::rfc_3339())
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_regex(false)
                .parse("")
                .wrap_err_with(|| eyre!("Parsing tracing filter failed!"))?,
        )
        .try_init()
        .map_err(|e| eyre!(e))
        .wrap_err_with(|| eyre!("Initializing tracing failed!"))?;
    info!("Running tokio runtime...");
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .wrap_err_with(|| eyre!("Failed to start tokio runtime"))?
        .block_on(start())?;

    Ok(())
}

async fn start() -> Result<()> {
    info!("Starting application...");
    Ok(())
}
