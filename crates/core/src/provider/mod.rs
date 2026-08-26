//! Shared Ethereum provider construction used by Safenet services.

use alloy::{
    network::AnyNetwork,
    primitives::U64,
    providers::{
        Provider as AlloyProvider, ProviderBuilder, ProviderCall, RootProvider, mock::Asserter,
    },
    rpc::{
        client::{ClientBuilder, NoParams},
        json_rpc::{RequestPacket, ResponsePacket},
    },
    transports::{BoxTransport, TransportError, TransportFut},
};
use std::{
    fmt,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};
use url::Url;

/// The outcome of an individual JSON-RPC request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcOutcome {
    /// The server returned a successful JSON-RPC response.
    Success,
    /// The transport failed, the server returned a JSON-RPC error, or no
    /// matching response was returned.
    Failure,
}

/// A callback invoked after each JSON-RPC request completes.
///
/// The callback receives only the method and bounded outcome. In particular,
/// it never receives URLs, request IDs, parameters, or response contents.
#[derive(Clone)]
pub struct RpcObserver(Arc<RpcObserverFn>);

type RpcObserverFn = dyn Fn(&str, RpcOutcome) + Send + Sync;

impl RpcObserver {
    /// Creates an observer from a request completion callback.
    pub fn new(observer: impl Fn(&str, RpcOutcome) + Send + Sync + 'static) -> Self {
        Self(Arc::new(observer))
    }

    fn observe(&self, method: &str, outcome: RpcOutcome) {
        (self.0)(method, outcome);
    }
}

impl fmt::Debug for RpcObserver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RpcObserver").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
struct RpcMetricsLayer {
    observer: RpcObserver,
}

impl Layer<BoxTransport> for RpcMetricsLayer {
    type Service = RpcMetricsTransport;

    fn layer(&self, inner: BoxTransport) -> Self::Service {
        RpcMetricsTransport {
            inner,
            observer: self.observer.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct RpcMetricsTransport {
    inner: BoxTransport,
    observer: RpcObserver,
}

impl Service<RequestPacket> for RpcMetricsTransport {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: RequestPacket) -> Self::Future {
        let requests = request
            .requests()
            .iter()
            .map(|request| (request.id().clone(), request.method().to_owned()))
            .collect::<Vec<_>>();
        let observer = self.observer.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let result = inner.call(request).await;
            match &result {
                Ok(response) => {
                    for (id, method) in requests {
                        let outcome = response
                            .responses()
                            .iter()
                            .find(|response| response.id == id)
                            .map_or(RpcOutcome::Failure, |response| {
                                if response.is_success() {
                                    RpcOutcome::Success
                                } else {
                                    RpcOutcome::Failure
                                }
                            });
                        observer.observe(&method, outcome);
                    }
                }
                Err(_) => {
                    for (_, method) in requests {
                        observer.observe(&method, RpcOutcome::Failure);
                    }
                }
            }
            result
        })
    }
}

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

    /// Connects to `url`, observing every JSON-RPC request made through the
    /// resulting provider, including the initial `eth_chainId` request.
    pub async fn connect_observed(
        url: &Url,
        observer: RpcObserver,
    ) -> Result<Self, TransportError> {
        let client = ClientBuilder::default()
            .layer(RpcMetricsLayer { observer })
            .connect(url.as_str())
            .await?;
        let root = RootProvider::new(client);
        let chain_id = root.get_chain_id().await?;
        Ok(Self { root, chain_id })
    }

    /// Creates a mocked provider.
    pub fn mocked(asserter: &Asserter) -> Self {
        Self::mocked_with_chain(asserter, 0x5afe)
    }

    /// Creates a mocked provider for a specific chain.
    pub fn mocked_with_chain(asserter: &Asserter, chain_id: u64) -> Self {
        let root = ProviderBuilder::default().connect_mocked_client(asserter.clone());
        Self { root, chain_id }
    }

    #[cfg(test)]
    fn mocked_observed(asserter: &Asserter, observer: RpcObserver) -> Self {
        let client = ClientBuilder::default()
            .layer(RpcMetricsLayer { observer })
            .transport(
                BoxTransport::new(alloy::transports::mock::MockTransport::new(
                    asserter.clone(),
                )),
                true,
            );
        let root = ProviderBuilder::default().connect_client(client);
        Self {
            root,
            chain_id: 0x5afe,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::rpc::{json_rpc::ErrorPayload, types::BlockNumberOrTag};
    use std::sync::Mutex;

    type Observations = Arc<Mutex<Vec<(String, RpcOutcome)>>>;

    fn observed_provider(asserter: &Asserter) -> (Provider, Observations) {
        let observations = Arc::new(Mutex::new(Vec::new()));
        let captured = observations.clone();
        let observer = RpcObserver::new(move |method, outcome| {
            captured.lock().unwrap().push((method.to_owned(), outcome));
        });
        (Provider::mocked_observed(asserter, observer), observations)
    }

    #[tokio::test]
    async fn observes_successful_and_json_rpc_error_responses() {
        let asserter = Asserter::new();
        let (provider, observations) = observed_provider(&asserter);

        asserter.push_success(&42_u64);
        assert_eq!(provider.get_block_number().await.unwrap(), 42);

        asserter.push_failure(ErrorPayload {
            code: -32_000,
            message: "failed".into(),
            data: None,
        });
        assert!(
            provider
                .get_block_by_number(BlockNumberOrTag::Latest)
                .await
                .is_err()
        );

        // An empty mock response queue fails at the transport layer.
        assert!(provider.get_block_number().await.is_err());

        assert_eq!(
            *observations.lock().unwrap(),
            [
                ("eth_blockNumber".to_owned(), RpcOutcome::Success),
                ("eth_getBlockByNumber".to_owned(), RpcOutcome::Failure),
                ("eth_blockNumber".to_owned(), RpcOutcome::Failure),
            ]
        );
    }
}
