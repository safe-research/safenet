use alloy::signers::k256::ecdsa::SigningKey;
use safenet_core::{driver, observability, tx::Signer};
use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;
use std::path::Path;
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
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The RPC endpoint used to initialize the chain provider.
    pub rpc: Url,
    /// The signer used to sign and submit transactions onchain.
    pub signer: Signer,
    /// The database URL backing persistent state and transaction storage.
    #[serde(with = "safenet_core::serialization::from_str")]
    pub database: SqliteConnectOptions,
    /// Observability (logging and metrics) configuration.
    #[serde(default)]
    pub observability: observability::Config,
    /// Configuration for the service driver and its components.
    #[serde(flatten)]
    pub driver: driver::Config,
}

impl Config {
    /// Loads a configuration from a file.
    pub async fn load(file: &Path) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).await?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc: "http://localhost:8545".parse().unwrap(),
            signer: SigningKey::from_slice(
                b"\xac\x09\x74\xbe\xc3\x9a\x17\xe3\x6b\xa4\xa6\xb4\xd2\x38\xff\x94\
                  \x4b\xac\xb4\x78\xcb\xed\x5e\xfc\xae\x78\x4d\x7b\xf4\xf2\xff\x80",
            )
            .map(Signer::new)
            .unwrap(),
            database: "sqlite::memory:".parse().unwrap(),
            observability: observability::Config::default(),
            driver: driver::Config::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::address;

    use super::*;

    #[test]
    fn deserializes_required_fields_and_flattened_driver_config() {
        // Only the required fields are set; the driver config (flattened
        // alongside them) and observability fall back to their defaults.
        let config = toml::from_str::<Config>("").unwrap();

        assert_eq!(config.rpc.as_str(), "http://localhost:8545/");
        assert_eq!(
            config.signer.address(),
            address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
        );
        // Connection options do not have useful introspection methods, as there
        // is a bug with `to_url_lossy` [1], hack something together for now.
        //
        // [1]: <https://github.com/transact-rs/sqlx/issues/4327>
        assert!(format!("{:?}", config.database).contains("in_memory: true"));
        assert_eq!(
            config.observability.log_filter.to_string(),
            observability::Config::default().log_filter.to_string()
        );
        assert_eq!(
            config.observability.metrics_address,
            observability::Config::default().metrics_address
        );
        assert_eq!(config.driver, driver::Config::default());

        // A field of a doubly-flattened component (the block watcher, reached
        // through the driver and indexer configs) is parsed into place.
        let config = toml::from_str::<Config>(
            r#"
                database = "sqlite:validator.db"

                [observability]
                log_filter = "validator=debug,info"

                [index]
                max_reorg_depth = 12

                [transactions]
                max_in_flight_transactions = 4
            "#,
        )
        .unwrap();

        assert_eq!(config.database.get_filename(), "validator.db");
        assert_eq!(
            config.observability.log_filter.to_string(),
            "validator=debug,info"
        );
        assert_eq!(config.driver.index.blocks.max_reorg_depth, 12);
        assert_eq!(config.driver.transactions.max_in_flight_transactions, 4);
    }
}
