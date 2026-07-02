mod bindings;
mod config;
mod service;

use self::{config::Config, service::DummyService};
use alloy::{primitives::Address, providers::ProviderBuilder};
use argh::FromArgs;
use safenet_core::{Driver, observability};
use sqlx::sqlite::SqlitePool;
use std::{error::Error, path::PathBuf};

#[derive(Debug, FromArgs)]
/// Safenet validator.
struct Options {
    /// path to the validator TOML configuration file.
    #[argh(option, default = "PathBuf::from(\"validator.toml\")")]
    config_file: PathBuf,

    /// print version information.
    #[argh(switch)]
    version: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let options: Options = argh::from_env();
    if options.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let config = Config::load(&options.config_file).await?;
    observability::init(config.observability)?;
    tracing::debug!(config_file = %options.config_file.display(), "validator configuration loaded");

    let provider = ProviderBuilder::new().connect(config.rpc.as_str()).await?;
    let pool = SqlitePool::connect_with(config.database).await?;

    // The watched contract addresses and events are placeholders until the real
    // validator service is implemented.
    let driver = Driver::new(
        DummyService,
        provider,
        config.signer,
        pool,
        vec![Address::ZERO],
        config.driver,
    )
    .await?;

    tracing::info!("starting validator service");
    driver.run().await;

    Ok(())
}
