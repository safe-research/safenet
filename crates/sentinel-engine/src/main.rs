mod api;
mod checkers;
mod config;
mod contracts;
mod engine;

use self::{
    checkers::{
        AddressPoisoningChecker, BaseChecker, BlocklistChecker, CancellationChecker, CowChecker,
        EscapeHatchChecker, ExcessiveApprovalChecker, NestedSafeChecker, RefundChecker,
        StakingChecker,
    },
    config::Config,
    engine::SentinelEngine,
};
use argh::FromArgs;
use safenet_core::{observability, provider::Provider, utils};
use std::{error::Error, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;

#[derive(Debug, FromArgs)]
/// Safenet sentinel engine.
struct Options {
    /// path to the sentinel engine TOML configuration file.
    #[argh(option, default = "PathBuf::from(\"sentinel-engine.toml\")")]
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
    let bind_address = config.bind_address;
    let rpc = config.rpc;
    let engine_config = config.engine;
    observability::init(config.observability)?;
    tracing::debug!(
        config_file = %options.config_file.display(),
        "sentinel engine configuration loaded"
    );

    let provider = Provider::connect(&rpc).await?;
    let address_poisoning = Arc::new(AddressPoisoningChecker::new(
        provider,
        engine_config.address_poisoning_lookback_blocks,
        engine_config.address_poisoning_max_block_range,
    ));
    let engine = SentinelEngine::new(vec![
        Box::new(CancellationChecker),
        Box::new(EscapeHatchChecker),
        Box::new(BaseChecker),
        Box::new(BlocklistChecker::new(engine_config.blocklist)),
        Box::new(NestedSafeChecker),
        Box::new(ExcessiveApprovalChecker),
        Box::new(CowChecker::new()),
        Box::new(StakingChecker),
        // RPC-backed, so they run last: cheaper local checkers above get a
        // chance to reach a verdict first. `RefundChecker` can only deny or
        // abstain (never affirm), so its position relative to
        // `address_poisoning` doesn't affect correctness, only which RPC
        // lookup runs first when both apply.
        Box::new(RefundChecker::new(address_poisoning.clone())),
        Box::new(address_poisoning),
    ]);

    let listener = TcpListener::bind(bind_address).await?;
    let local_address = listener.local_addr()?;

    tracing::info!(%local_address, "starting sentinel engine");
    tokio::select! {
        result = axum::serve(listener, api::router(engine)) => result?,
        _ = utils::shutdown_signal() => {
            tracing::info!("received shutdown signal; stopping sentinel engine");
        }
    }

    Ok(())
}
