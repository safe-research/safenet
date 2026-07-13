mod bindings;
mod config;
mod consensus;
mod frost;
mod merkle;
mod secrets;
mod service;
mod state;

use self::{
    bindings::Consensus,
    config::Config,
    service::{Action, ValidatorService},
};
use alloy::providers::{Provider, ProviderBuilder};
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
    let chain_id = provider.get_chain_id().await?;
    tracing::debug!(chain_id, %consensus, %coordinator, "resolved onchain contracts");

    let mut watched = vec![consensus, coordinator];
    watched.extend(config.validator.oracles.iter().copied());

    let account = config.signer.address();
    let staker = config.validator.staker;

    let service = ValidatorService::new(
        chain_id,
        account,
        pool.clone(),
        coordinator,
        config.validator,
    )
    .await?;

    let mut driver = Driver::new(
        service,
        provider.clone(),
        config.signer,
        pool,
        watched,
        config.driver,
    )
    .await?;

    // Reconcile the onchain staker association before starting the driver.
    if let Some(staker) = staker {
        let current_staker = Consensus::new(consensus, &provider)
            .getValidatorStaker(account)
            .call()
            .await?;
        if current_staker != staker {
            tracing::info!(%account, %staker, "reconciling validator staker onchain");
            driver
                .queue_action(Action::SetValidatorStaker { staker })
                .await?;
        }
    }

    tracing::info!("starting validator service");
    driver.run().await;

    Ok(())
}
