mod bindings;
mod config;
mod state;
mod watcher;

use argh::FromArgs;
use config::ValidatorConfig;
use state::ValidatorState;
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli: Cli = argh::from_env();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&cli.log_level)?)
        .init();

    debug!(config_file = %cli.config_file.display(), "loading validator configuration");
    let config_toml = std::fs::read_to_string(&cli.config_file)?;
    let config = ValidatorConfig::from_toml(&config_toml)?;

    info!("validator configuration loaded");
    let mut state = ValidatorState::default();
    watcher::run(&config, &mut state).await?;
    Ok(())
}
