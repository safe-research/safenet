use crate::config::provider::Provider;
use alloy::providers::Provider as _;
use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Gnosis,
    Sepolia,
    Anvil,
}

impl Chain {
    pub async fn load(provider: &Provider) -> Result<Self> {
        let chain_id = provider.get_chain_id().await?;
        Chain::from_chain_id(chain_id)
    }

    pub fn from_chain_id(chain_id: u64) -> Result<Self> {
        match chain_id {
            100 => Ok(Self::Gnosis),
            11155111 => Ok(Self::Sepolia),
            31337 => Ok(Self::Anvil),
            _ => bail!("unsupported chain ID: {chain_id}"),
        }
    }

    pub fn id(&self) -> u64 {
        match self {
            Self::Gnosis => 100,
            Self::Sepolia => 11155111,
            Self::Anvil => 31337,
        }
    }

    pub fn blocks_per_epoch(&self) -> u64 {
        match self {
            Self::Gnosis => 1440,
            Self::Sepolia => 600,
            Self::Anvil => 60,
        }
    }
}
