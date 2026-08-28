use alloy::primitives::Address;
use safenet_core::observability;
use serde::Deserialize;
use std::{net::SocketAddr, num::NonZeroU64, path::Path};
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
    /// Transaction destinations that are always considered insecure.
    pub blocklist: Vec<Address>,
    /// Number of blocks inspected for a prior interaction with a candidate
    /// address.
    pub address_poisoning_lookback_blocks: u64,
    /// The widest `toBlock - fromBlock` *span* the configured RPC allows in
    /// a single `eth_getLogs` call — the same number reported in a
    /// provider's own "range exceeds limit" error, **not** a block count
    /// (a call from block 100 to 110 has a span of 10 but covers 11 block
    /// numbers; if a provider documents its cap as an inclusive block count
    /// `N`, use `N - 1` here). Unset by default, which issues the whole
    /// `address_poisoning_lookback_blocks` window as a single call; set
    /// this when the provider caps it below that, and the lookback is
    /// split into consecutive calls of at most this width to still cover
    /// the full window. Must be non-zero if set.
    #[serde(default)]
    pub address_poisoning_max_block_range: Option<NonZeroU64>,
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
        blocklist = ["0x0404040404040404040404040404040404040404"]
        address_poisoning_lookback_blocks = 50000
    "#;

    #[test]
    fn deserializes_required_fields_and_defaults_the_rest() {
        let config = toml::from_str::<Config>(TOML).unwrap();

        assert_eq!(config.rpc.as_str(), "https://eth.llamarpc.com/");
        assert_eq!(config.bind_address, "127.0.0.1:5473".parse().unwrap());
        assert_eq!(config.engine.blocklist, [Address::repeat_byte(0x04)]);
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
                blocklist = []
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
                    blocklist = []
                    address_poisoning_lookback_blocks = 50000
                "#,
            )
            .is_err()
        );
    }

    #[test]
    fn parses_sample_config() {
        // The sample linked from the sentinel engine guide must stay a valid,
        // loadable example of the schema above.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("sentinel-engine.sample.toml");
        let contents = std::fs::read_to_string(path).unwrap();
        toml::from_str::<Config>(&contents).unwrap();
    }
}
