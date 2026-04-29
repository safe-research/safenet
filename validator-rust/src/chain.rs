use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Gnosis,
    Sepolia,
    Anvil,
}

impl Chain {
    pub fn new(chain_id: u64) -> Result<Self> {
        match chain_id {
            100 => Ok(Self::Gnosis),
            11155111 => Ok(Self::Sepolia),
            31337 => Ok(Self::Anvil),
            _ => bail!("unsupported chain ID: {chain_id}"),
        }
    }

    pub fn blocks_per_epoch(self) -> u64 {
        match self {
            Self::Gnosis => 1440,
            Self::Sepolia => 600,
            Self::Anvil => 60,
        }
    }
}
