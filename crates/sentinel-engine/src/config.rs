use safenet_core::observability;
use serde::Deserialize;
use std::path::Path;
use tokio::{fs, io};

/// Error produced when loading the configuration.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An IO error when interacting with the filesystem.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Error when parsing the configuration.
    #[error(transparent)]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Observability (logging and metrics) configuration.
    #[serde(default)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};

    #[test]
    fn deserializes_required_fields_and_defaults_the_rest() {
        let config = toml::from_str::<Config>(
            r#"
            "#,
        )
        .unwrap();

        assert_eq!(
            config.observability.log_filter.to_string(),
            observability::Config::default().log_filter.to_string()
        );
        assert_eq!(
            config.observability.metrics_address,
            observability::Config::default().metrics_address
        );
    }

    #[test]
    fn deserializes_observability_overrides() {
        let config = toml::from_str::<Config>(
            r#"
                [observability]
                log_filter = "sentinel_engine=debug,info"
                metrics_address = "127.0.0.1:9090"
            "#,
        )
        .unwrap();

        assert_eq!(
            config.observability.log_filter.to_string(),
            "sentinel_engine=debug,info"
        );
        assert_eq!(
            config.observability.metrics_address,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 9090))
        );
    }

    #[test]
    fn rejects_unknown_field() {
        assert!(
            toml::from_str::<Config>(
                r#"
                    invalid_field = "foo"
                "#,
            )
            .is_err()
        );
    }
}
