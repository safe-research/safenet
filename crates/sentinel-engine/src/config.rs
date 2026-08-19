use safenet_core::observability;
use serde::Deserialize;
use std::{net::SocketAddr, path::Path};
use tokio::{fs, io};
use url::Url;

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
    /// The RPC endpoint used by checks that query onchain state.
    pub rpc: Url,
    /// The address on which the transaction-checking API listens.
    #[serde(default = "default_bind_address")]
    pub bind_address: SocketAddr,
    /// Observability (logging and metrics) configuration.
    #[serde(default)]
    pub observability: observability::Config,
    /// Configuration for the engine's transaction checks.
    pub engine: EngineConfig,
}

/// Configuration for the engine's transaction checks.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineConfig {
    /// Number of blocks inspected for a prior interaction with a candidate
    /// address.
    pub address_poisoning_lookback_blocks: u64,
}

fn default_bind_address() -> SocketAddr {
    ([127, 0, 0, 1], 5473).into()
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

    const TOML: &str = r#"
        rpc = "https://eth.llamarpc.com"

        [engine]
        address_poisoning_lookback_blocks = 50000
    "#;

    #[test]
    fn deserializes_required_fields_and_defaults_the_rest() {
        let config = toml::from_str::<Config>(TOML).unwrap();

        assert_eq!(config.rpc.as_str(), "https://eth.llamarpc.com/");
        assert_eq!(config.bind_address, "127.0.0.1:5473".parse().unwrap());
        assert_eq!(config.engine.address_poisoning_lookback_blocks, 50_000);
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
                rpc = "https://eth.llamarpc.com"
                bind_address = "0.0.0.0:8080"

                [observability]
                log_filter = "sentinel_engine=debug,info"
                metrics_address = "127.0.0.1:9090"

                [engine]
                address_poisoning_lookback_blocks = 50000
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
                    rpc = "https://eth.llamarpc.com"
                    invalid_field = "foo"

                    [engine]
                    address_poisoning_lookback_blocks = 50000
                "#,
            )
            .is_err()
        );
    }
}
