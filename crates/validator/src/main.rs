mod config;

use self::config::Config;
use argh::FromArgs;
use safenet_core::observability;
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
    Ok(())
}
