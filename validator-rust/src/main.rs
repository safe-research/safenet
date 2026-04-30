mod actions;
mod bindings;
mod config;
mod driver;
mod frost;
mod state;
mod watcher;

use self::config::ValidatorConfig;
use anyhow::Result;
use argh::FromArgs;
use std::path::PathBuf;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, FromArgs)]
/// Safenet validator.
struct Cli {
    /// tracing env-filter used for logging.
    #[argh(option, default = "String::from(\"info\")")]
    log_level: String,

    /// path to the validator TOML configuration file.
    #[argh(option, default = "PathBuf::from(\"validator.toml\")")]
    config_file: PathBuf,

    /// print version information.
    #[argh(switch)]
    version: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = argh::from_env();

    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&cli.log_level)?)
        .init();

    debug!(config_file = %cli.config_file.display(), "loading validator configuration");
    let config_toml = std::fs::read_to_string(&cli.config_file)?;
    let config = ValidatorConfig::from_toml(&config_toml)?;

    info!("validator configuration loaded");
    driver::run(config).await
}
