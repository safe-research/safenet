mod action;
mod bindings;
mod config;
mod dynamic_checker;
mod effect;
mod hashing;
mod service;
mod state;
mod static_checker;

use self::{
    config::Config, dynamic_checker::RemoteChecker, service::SentinelService,
    static_checker::StaticChecker,
};
use alloy::{
    primitives::U256,
    providers::{Provider, ProviderBuilder},
};
use argh::FromArgs;
use safenet_core::{Driver, observability};
use sqlx::sqlite::SqlitePool;
use std::{error::Error, path::PathBuf};

#[derive(Debug, FromArgs)]
/// Safenet sentinel.
struct Options {
    /// path to the sentinel TOML configuration file.
    #[argh(option, default = "PathBuf::from(\"sentinel.toml\")")]
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
    tracing::debug!(config_file = %options.config_file.display(), "sentinel configuration loaded");

    let provider = ProviderBuilder::new().connect(config.rpc.as_str()).await?;
    let pool = SqlitePool::connect_with(config.database).await?;
    let chain_id = U256::from(provider.get_chain_id().await?);

    let service = SentinelService::new(
        config.oracle,
        config.sentinel.fee_token,
        config.consensus,
        config.signer.clone(),
        chain_id,
        config.sentinel.voting_window,
        StaticChecker::new(config.sentinel.blocklist),
        RemoteChecker::new(config.sentinel.remote_check_url),
    );

    let driver = Driver::new(
        service,
        provider,
        config.signer,
        pool,
        vec![config.oracle, config.consensus],
        config.driver,
    )
    .await?;

    tracing::info!("starting sentinel service");
    driver.run().await;

    Ok(())
}
