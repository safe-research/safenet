mod action;
mod bindings;
mod config;
mod effect;
mod engine;
mod hashing;
mod service;
mod state;

use self::{config::Config, engine::EngineClient, service::SentinelService};
use alloy::primitives::U256;
use argh::FromArgs;
use safenet_core::{Driver, observability, provider::Provider, utils};
use std::{error::Error, path::PathBuf, time::Duration};

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

    let provider = Provider::connect(&config.rpc).await?;
    let pool = utils::connect_sqlite(config.database).await?;
    let chain_id = provider.chain_id();

    // TODO: Derive this from effect lifecycle data so time elapsed before the
    // effect starts can be deducted from the request's actual reveal deadline.
    // For now, use three quarters of the configured time until the block
    // before the reveal deadline to leave some wiggle room for delays, with a
    // one-second minimum for practical deployments.
    let engine_timeout = {
        let block_time = config.driver.index.blocks.block_time.resolve(chain_id)?;
        Duration::from_millis(
            u64::try_from(
                u128::from(config.sentinel.voting_window.saturating_sub(1))
                    .saturating_mul(u128::from(block_time))
                    .saturating_mul(3)
                    / 4,
            )
            .unwrap_or(u64::MAX)
            .max(1_000),
        )
    };

    let service = SentinelService::new(
        config.oracle,
        config.sentinel.fee_token,
        config.consensus,
        config.signer.clone(),
        U256::from(chain_id),
        config.sentinel.voting_window,
        EngineClient::new(config.sentinel.engine)?,
        engine_timeout,
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
