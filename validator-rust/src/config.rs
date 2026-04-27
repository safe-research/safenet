use alloy::primitives::{Address, B256};
use serde::Deserialize;
use url::Url;

use crate::chain::Chain;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub address: Address,
    #[serde(default)]
    pub active_from: u64,
    pub active_before: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorConfig {
    pub storage_file: Option<String>,
    pub rpc_url: Url,
    pub private_key: B256,
    pub staker_address: Option<Address>,
    pub consensus_address: Address,
    pub coordinator_address: Address,
    pub chain: Chain,
    pub participants: Vec<Participant>,
    pub genesis_salt: Option<B256>,
    pub blocks_per_epoch: Option<u64>,
    pub blocks_before_resubmit: Option<u64>,
    pub base_fee_multiplier: Option<f64>,
    pub priority_fee_per_gas: Option<u64>,
    pub block_time_override: Option<u64>,
    pub max_reorg_depth: Option<u64>,
    pub block_page_size: Option<u64>,
    pub block_all_logs_query_retry_count: Option<u64>,
    pub block_single_query_retry_count: Option<u64>,
    pub key_gen_timeout: Option<u64>,
    pub signing_timeout: Option<u64>,
    pub max_logs_per_query: Option<u64>,
    pub skip_genesis: Option<bool>,
}

impl ValidatorConfig {
    pub fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"
rpc_url = "http://127.0.0.1:8545"
private_key = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"
consensus_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
coordinator_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
chain = "gnosis"

[[participants]]
address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

[[participants]]
address = "0x6Adb3baB5730852eB53987EA89D8e8f16393C200"
active_from = 10
active_before = 20
"#;

    #[test]
    fn parses_valid_config_with_defaults() {
        let config = ValidatorConfig::from_toml(VALID_CONFIG).expect("config parses");

        assert_eq!(config.chain, Chain::Gnosis);
        assert_eq!(config.genesis_salt, None);
        assert_eq!(config.blocks_per_epoch, None);
        assert_eq!(config.participants.len(), 2);
        assert_eq!(config.participants[0].active_from, 0);
        assert_eq!(config.participants[1].active_from, 10);
        assert_eq!(config.participants[1].active_before, Some(20));
        assert_eq!(config.skip_genesis, None);
    }

    #[test]
    fn rejects_unknown_chains() {
        let input = VALID_CONFIG.replace("chain = \"gnosis\"", "chain = \"mainnet\"");
        let error = ValidatorConfig::from_toml(&input).expect_err("config fails");

        assert!(error.to_string().contains("unknown variant `mainnet`"));
    }

    #[test]
    fn rejects_log_level_in_config_file() {
        let input = format!("log_level = \"debug\"\n{VALID_CONFIG}");
        let error = ValidatorConfig::from_toml(&input).expect_err("config fails");

        assert!(error.to_string().contains("unknown field `log_level`"));
    }

    #[test]
    fn parses_optional_values() {
        let input = VALID_CONFIG.replacen(
            "[[participants]]",
            "blocks_per_epoch = 42\nskip_genesis = true\n\n[[participants]]",
            1,
        );
        let config = ValidatorConfig::from_toml(&input).expect("config parses");

        assert_eq!(config.blocks_per_epoch, Some(42));
        assert_eq!(config.skip_genesis, Some(true));
    }
}
