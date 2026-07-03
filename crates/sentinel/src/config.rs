use alloy::primitives::Address;
use safenet_core::{driver, observability, tx::Signer};
use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;
use std::path::Path;
use tokio::{fs, io};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The RPC endpoint used to initialize the chain provider.
    pub rpc: Url,
    /// The signer used to sign and submit transactions onchain.
    pub signer: Signer,
    /// The database URL backing persistent state and transaction storage.
    #[serde(with = "safenet_core::serialization::from_str")]
    pub database: SqliteConnectOptions,
    /// The `SentinelOracle` contract watched and voted/committed on.
    pub oracle: Address,
    /// The `Consensus` contract whose proposals are hashed into request ids.
    pub consensus: Address,
    /// Configuration for the sentinel's own detection and voting logic.
    pub sentinel: SentinelConfig,
    /// Observability (logging and metrics) configuration.
    #[serde(default)]
    pub observability: observability::Config,
    /// Configuration for the service driver and its components.
    #[serde(flatten)]
    pub driver: driver::Config,
}

/// Configuration specific to the sentinel's request handling, as opposed to
/// the infrastructure it shares with other Safenet services.
//
// TODO(epic Phase E2, follow-up): pick and document sensible defaults for
// `voting_window` and `blocklist` (`fee_token`/`oracle`/`consensus` are
// deployment-specific and should stay required) once the sentinel's config
// shape has settled; for now all three are mandatory so a missing value fails
// loudly rather than silently watching the wrong address or window.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SentinelConfig {
    /// The ERC-20 fee token approved for bonds.
    pub fee_token: Address,
    /// The number of blocks a `Preparing` request is kept alive for before
    /// being cleaned up.
    pub voting_window: u64,
    /// Transaction destinations that are always denied.
    pub blocklist: Vec<Address>,
}

impl Config {
    pub async fn load(file: &Path) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).await?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    const TOML: &str = r#"
        rpc = "https://eth.llamarpc.com"
        signer = "0x0000000000000000000000000000000000000000000000000000000000000001"
        database = "sqlite:sentinel.db"
        oracle = "0x0101010101010101010101010101010101010101"
        consensus = "0x0202020202020202020202020202020202020202"

        [sentinel]
        fee_token = "0x0303030303030303030303030303030303030303"
        voting_window = 100
        blocklist = ["0x0404040404040404040404040404040404040404"]
    "#;

    #[test]
    fn deserializes_required_fields_and_defaults_the_rest() {
        // Observability and the flattened driver config are both omitted and
        // fall back to their own defaults, matching the `validator` crate's
        // config convention.
        let config = toml::from_str::<Config>(TOML).unwrap();

        assert_eq!(config.rpc.as_str(), "https://eth.llamarpc.com/");
        assert_eq!(config.database.get_filename(), "sentinel.db");
        assert_eq!(
            config.oracle,
            address!("0x0101010101010101010101010101010101010101")
        );
        assert_eq!(
            config.consensus,
            address!("0x0202020202020202020202020202020202020202")
        );
        assert_eq!(
            config.sentinel.fee_token,
            address!("0x0303030303030303030303030303030303030303")
        );
        assert_eq!(config.sentinel.voting_window, 100);
        assert_eq!(
            config.sentinel.blocklist,
            vec![address!("0x0404040404040404040404040404040404040404")]
        );
        assert_eq!(
            config.observability.log_filter.to_string(),
            observability::Config::default().log_filter.to_string()
        );
        assert_eq!(config.driver, driver::Config::default());
    }

    #[test]
    fn rejects_config_missing_a_deployment_specific_field() {
        // `oracle`, `consensus` and the `[sentinel]` block have no sensible
        // default and must fail loudly rather than silently defaulting to the
        // zero address (see the `SentinelConfig` TODO above).
        let without_oracle = TOML.replacen(
            r#"oracle = "0x0101010101010101010101010101010101010101""#,
            "",
            1,
        );
        assert!(toml::from_str::<Config>(&without_oracle).is_err());
    }
}
