mod bindings;
mod config;
mod consensus;
mod frost;
mod merkle;
mod secrets;
mod service;
mod state;

use self::{bindings::Consensus, config::Config, service::ValidatorService};
use alloy::providers::ProviderBuilder;
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

    let consensus = config.validator.consensus;
    let coordinator = Consensus::new(consensus, &provider)
        .getCoordinator()
        .call()
        .await?;
    tracing::debug!(%consensus, %coordinator, "resolved onchain contracts");

    let mut watched = vec![consensus, coordinator];
    watched.extend(config.validator.oracles.iter().copied());

    let driver = Driver::new(
        ValidatorService,
        provider,
        config.signer,
        pool,
        watched,
        config.driver,
    )
    .await?;

    tracing::info!("starting validator service");
    driver.run().await;

    Ok(())
}
