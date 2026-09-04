//! Shared Ethereum provider construction used by Safenet services.

use crate::{
    metrics::{self, RpcRequestResult},
    utils::Json,
};
use alloy::{
    network::AnyNetwork,
    primitives::U64,
    providers::{Provider as AlloyProvider, ProviderCall, RootProvider},
    rpc::{
        client::{ClientBuilder, NoParams},
        json_rpc::{RequestPacket, ResponsePacket},
    },
    transports::{BoxTransport, TransportError, TransportFut},
};
#[cfg(any(test, feature = "test-util"))]
use alloy::{providers::ProviderBuilder, transports::mock::Asserter};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    task::{Context, Poll},
};
use tower::{Layer, Service};
use url::Url;

#[derive(Clone, Copy, Debug)]
struct ObservabilityLayer;

impl Layer<BoxTransport> for ObservabilityLayer {
    type Service = ObservabilityTransport;

    fn layer(&self, inner: BoxTransport) -> Self::Service {
        ObservabilityTransport { inner }
    }
}

#[derive(Clone, Debug)]
struct ObservabilityTransport {
    inner: BoxTransport,
}

struct ResponseJson<'a>(&'a ResponsePacket);

impl<'a> Debug for ResponseJson<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Work around the fact that `ResponsePacket` does not implement
        // `serde::Serialize` and implement `Debug` for it directly forwarding
        // the actual writing to the `Json` formatter implementation.
        match self.0 {
            ResponsePacket::Single(response) => write!(f, "{}", Json(response)),
            ResponsePacket::Batch(responses) => write!(f, "{}", Json(responses)),
        }
    }
}

impl Service<RequestPacket> for ObservabilityTransport {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request_packet: RequestPacket) -> Self::Future {
        let mut statuses = request_packet
            .requests()
            .iter()
            .map(|request| {
                (
                    request.id().clone(),
                    (request.method().to_owned(), RpcRequestResult::Failure),
                )
            })
            .collect::<HashMap<_, _>>();
        if statuses.len() != request_packet.len() {
            // `alloy` guarantees that IDs are unique internally, but just in
            // case this assumption stops being true, log an error so we know
            // that we need to re-evaluate how we track requests.
            tracing::error!("unexpected duplicate request ID; metrics may be inaccurate");
        }

        let mut inner = self.inner.clone();
        Box::pin(async move {
            tracing::trace!(
                request = %Json(&request_packet),
                "sending JSON-RPC request"
            );
            let response_packet = inner.call(request_packet).await;
            tracing::trace!(
                response = ?response_packet.as_ref().map(ResponseJson),
                "received JSON-RPC response"
            );
            if let Ok(response_packet) = &response_packet {
                for response in response_packet.responses() {
                    let Some((_, result)) = statuses.get_mut(&response.id) else {
                        tracing::warn!(
                            id = ?response.id,
                            "unexpected response without matching request"
                        );
                        continue;
                    };

                    *result = if response.is_success() {
                        RpcRequestResult::Success
                    } else {
                        RpcRequestResult::Failure
                    };
                }
            }
            for (_, (method, result)) in statuses {
                metrics::rpc_requests_total(&method, result).increment(1);
            }
            response_packet
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
        let client = ClientBuilder::default()
            .layer(ObservabilityLayer)
            .connect(url.as_str())
            .await?;
        let root = RootProvider::new(client);
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
