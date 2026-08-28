//! Shared Ethereum provider construction used by Safenet services.

use alloy::{
    network::AnyNetwork,
    primitives::U64,
    providers::{Provider as AlloyProvider, ProviderCall, RootProvider},
    rpc::client::NoParams,
    transports::TransportError,
};
#[cfg(any(test, feature = "test-util"))]
use alloy::{providers::ProviderBuilder, transports::mock::Asserter};
use url::Url;

/// The standard [`alloy`] provider used by Safenet services.
#[derive(Clone, Debug)]
pub struct Provider {
    root: RootProvider<AnyNetwork>,
    chain_id: u64,
}

impl Provider {
    /// Connects to `url`.
    pub async fn connect(url: &Url) -> Result<Self, TransportError> {
        let root = RootProvider::connect(url.as_str()).await?;
        let chain_id = root.get_chain_id().await?;
        Ok(Self { root, chain_id })
    }

    /// Creates a mocked provider.
    #[cfg(any(test, feature = "test-util"))]
    pub fn mocked(asserter: &Asserter) -> Self {
        Self::mocked_with_chain(asserter, 0x5afe)
    }

    /// Creates a mocked provider for a specific chain.
    #[cfg(any(test, feature = "test-util"))]
    pub fn mocked_with_chain(asserter: &Asserter, chain_id: u64) -> Self {
        let root = ProviderBuilder::default().connect_mocked_client(asserter.clone());
        Self { root, chain_id }
    }

    /// Returns the chain ID read when this provider connected.
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

impl AlloyProvider<AnyNetwork> for Provider {
    fn root(&self) -> &RootProvider<AnyNetwork> {
        &self.root
    }

    fn get_chain_id(&self) -> ProviderCall<NoParams, U64, u64> {
        ProviderCall::Ready(Some(Ok(self.chain_id)))
    }
}
