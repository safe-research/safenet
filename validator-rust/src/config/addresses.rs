use crate::{bindings::Consensus, config::provider::Provider};
use alloy::primitives::Address;
use anyhow::Result;

#[derive(Clone, Copy)]
pub struct Addresses {
    pub consensus: Address,
    pub coordinator: Address,
}

impl Addresses {
    pub async fn load(provider: &Provider, consensus: Address) -> Result<Self> {
        let coordinator = Consensus::new(consensus, provider)
            .getCoordinator()
            .call()
            .await?;
        Ok(Addresses {
            consensus,
            coordinator,
        })
    }
}
