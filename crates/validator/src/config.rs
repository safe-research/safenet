use safenet_core::observability;
use serde::Deserialize;
use std::path::Path;
use tokio::{fs, io};

/// Error produced by the block watcher.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An IO error when interacting with the filesystem.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Error when parsing the configuration.
    #[error(transparent)]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub observability: observability::Config,
}

impl Config {
    /// Loads a configuration from a file.
    pub async fn load(file: &Path) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).await?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}
