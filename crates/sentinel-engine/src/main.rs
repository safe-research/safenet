mod api;
mod config;
mod engine;

use self::{config::Config, engine::SentinelEngine};
use argh::FromArgs;
use safenet_core::{observability, utils};
use std::{error::Error, path::PathBuf};
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
    observability::init(config.observability)?;
    tracing::debug!(
        config_file = %options.config_file.display(),
        "sentinel engine configuration loaded"
    );

    let listener = TcpListener::bind(bind_address).await?;
    let local_address = listener.local_addr()?;

    tracing::info!(%local_address, "starting sentinel engine");
    tokio::select! {
        result = axum::serve(listener, api::router(SentinelEngine)) => result?,
        _ = utils::shutdown_signal() => {
            tracing::info!("received shutdown signal; stopping sentinel engine");
        }
    }

    Ok(())
}
