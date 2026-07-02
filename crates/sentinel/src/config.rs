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
    pub observability: observability::Config,
    /// Configuration for the service driver and its components.
    #[serde(flatten)]
    pub driver: driver::Config,
}

/// Configuration specific to the sentinel's request handling, as opposed to
/// the infrastructure it shares with other Safenet services.
// TODO(epic Phase E2): flesh this out further, e.g. its own deserialization
// tests and defaults, once the sentinel's config shape has settled.
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
