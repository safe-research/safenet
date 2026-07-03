#![cfg_attr(not(test), expect(dead_code))]

use alloy::primitives::{Address, B256};
use safenet_core::{driver, observability, tx::Signer};
use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;
use std::{num::NonZeroU64, path::Path};
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
    /// The RPC endpoint used to initialize the chain provider.
    pub rpc: Url,
    /// The signer used to sign and submit transactions onchain.
    pub signer: Signer,
    /// The database URL backing persistent state and transaction storage.
    #[serde(with = "safenet_core::serialization::from_str")]
    pub database: SqliteConnectOptions,
    /// Configuration specific to the validator service and its consensus
    /// participation.
    pub validator: ValidatorConfig,
    /// Observability (logging and metrics) configuration.
    #[serde(default)]
    pub observability: observability::Config,
    /// Configuration for the service driver and its components.
    #[serde(default, flatten)]
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

/// Configuration for the validator service itself: the contract it follows, the
/// consensus group it participates in, and the timeouts governing its state
/// machine.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorConfig {
    /// The `Consensus` contract the validator proposes epochs and attests
    /// transactions on.
    pub consensus: Address,
    /// The staker address to associate with this validator account onchain. It
    /// is reconciled against the onchain value on startup.
    pub staker: Option<Address>,
    /// The validator set participating in key generation and signing, each with
    /// the epoch window during which it is active.
    #[serde(default)]
    pub participants: Vec<Participant>,
    /// The oracle contracts whose results the validator honors when signing
    /// oracle transactions.
    #[serde(default)]
    pub oracles: Vec<Address>,
    /// The salt mixed into the genesis group's key generation context.
    pub genesis_salt: Option<B256>,
    /// The number of blocks in an epoch, controlling epoch rollover timing.
    #[serde(default = "ValidatorConfig::default_blocks_per_epoch")]
    pub blocks_per_epoch: NonZeroU64,
    /// The number of blocks a distributed key generation ceremony may run
    /// before timing out.
    #[serde(default = "ValidatorConfig::default_keygen_timeout")]
    pub key_gen_timeout: NonZeroU64,
    /// The number of blocks a signing ceremony may run before timing out.
    #[serde(default = "ValidatorConfig::default_signing_timeout")]
    pub signing_timeout: NonZeroU64,
    /// The number of blocks to wait for an oracle result before timing out.
    #[serde(default = "ValidatorConfig::default_oracle_timeout")]
    pub oracle_timeout: NonZeroU64,
}

impl ValidatorConfig {
    const fn default_blocks_per_epoch() -> NonZeroU64 {
        NonZeroU64::new(1440).unwrap()
    }

    const fn default_keygen_timeout() -> NonZeroU64 {
        NonZeroU64::new(120).unwrap()
    }

    const fn default_signing_timeout() -> NonZeroU64 {
        NonZeroU64::new(6).unwrap()
    }

    const fn default_oracle_timeout() -> NonZeroU64 {
        NonZeroU64::new(12).unwrap()
    }
}

/// A validator participating in the consensus group, along with the epoch
/// window during which it takes part in key generation and signing.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    /// The participant's account address.
    pub address: Address,
    /// The first epoch the participant is active from (inclusive).
    #[serde(default)]
    pub active_from: u64,
    /// The epoch the participant becomes inactive before (exclusive), if it
    /// ever leaves the set.
    #[serde(default)]
    pub active_before: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, b256};

    #[test]
    fn deserializes_required_fields() {
        // Only the required fields are set; other settings fall back to their
        // default values.
        let config = toml::from_str::<Config>(
            r#"
                rpc = "http://localhost:8545"
                signer = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                database = "sqlite::memory:"

                [validator]
                consensus = "0x1111111111111111111111111111111111111111"
            "#,
        )
        .unwrap();

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
            config.validator.consensus,
            address!("0x1111111111111111111111111111111111111111")
        );
    }

    #[test]
    fn deserializes_validator_section() {
        let config = toml::from_str::<Config>(
            r#"
                rpc = "http://localhost:8545"
                signer = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                database = "sqlite::memory:"

                [validator]
                consensus = "0x1111111111111111111111111111111111111111"
                staker = "0x3333333333333333333333333333333333333333"
                genesis_salt = "0x00000000000000000000000000000000000000000000000000000000000000ab"
                blocks_per_epoch = 100
                key_gen_timeout = 20
                signing_timeout = 30
                oracle_timeout = 40
                oracles = [
                    "0x4444444444444444444444444444444444444444",
                    "0x5555555555555555555555555555555555555555",
                ]

                [[validator.participants]]
                address = "0x6666666666666666666666666666666666666666"

                [[validator.participants]]
                address = "0x7777777777777777777777777777777777777777"
                active_from = 5
                active_before = 9
            "#,
        )
        .unwrap();

        let validator = &config.validator;
        assert_eq!(
            validator.consensus,
            address!("0x1111111111111111111111111111111111111111")
        );
        assert_eq!(
            validator.staker.unwrap(),
            address!("0x3333333333333333333333333333333333333333")
        );
        assert_eq!(
            validator.genesis_salt.unwrap(),
            b256!("0x00000000000000000000000000000000000000000000000000000000000000ab")
        );
        assert_eq!(validator.blocks_per_epoch.get(), 100);
        assert_eq!(validator.key_gen_timeout.get(), 20);
        assert_eq!(validator.signing_timeout.get(), 30);
        assert_eq!(validator.oracle_timeout.get(), 40);
        assert_eq!(
            validator.oracles,
            [
                address!("0x4444444444444444444444444444444444444444"),
                address!("0x5555555555555555555555555555555555555555"),
            ]
        );

        // The first participant omits its epoch window, defaulting to active
        // from genesis with no end; the second sets an explicit window.
        assert_eq!(validator.participants.len(), 2);
        assert_eq!(
            validator.participants[0].address,
            address!("0x6666666666666666666666666666666666666666")
        );
        assert_eq!(validator.participants[0].active_from, 0);
        assert_eq!(validator.participants[0].active_before, None);
        assert_eq!(
            validator.participants[1].address,
            address!("0x7777777777777777777777777777777777777777")
        );
        assert_eq!(validator.participants[1].active_from, 5);
        assert_eq!(validator.participants[1].active_before, Some(9));
    }

    #[test]
    fn deserializes_with_optional_service_fields() {
        let config = toml::from_str::<Config>(
            r#"
                    rpc = "https://eth.llamarpc.com"
                    signer = "0x0000000000000000000000000000000000000000000000000000000000000001"
                    database = "sqlite:validator.db"

                    [validator]
                    consensus = "0x0000000000000000000000000000000000000000"

                    [observability]
                    log_filter = "validator=debug,info"

                    [index]
                    max_reorg_depth = 12

                    [transactions]
                    max_in_flight_transactions = 4
                "#,
        )
        .unwrap();

        assert_eq!(config.rpc.as_str(), "https://eth.llamarpc.com/");
        // Computed with:
        //
        // ```
        // import { privateKeyToAddress } from "viem/accounts";
        // console.log(privateKeyToAddress(
        //     "0x0000000000000000000000000000000000000000000000000000000000000001",
        // ));
        // ```
        assert_eq!(
            config.signer.address(),
            address!("0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf")
        );
        assert_eq!(config.database.get_filename(), "validator.db");
        assert_eq!(config.validator.consensus, Address::ZERO);
        assert_eq!(
            config.observability.log_filter.to_string(),
            "validator=debug,info"
        );
        assert_eq!(config.driver.index.blocks.max_reorg_depth, 12);
        assert_eq!(config.driver.transactions.max_in_flight_transactions, 4);
    }
}
