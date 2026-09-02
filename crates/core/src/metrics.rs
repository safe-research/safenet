//! This crate's own Prometheus metrics.
//!
//! Each metric's name and its expected labels are defined together in one
//! accessor function here, so every place a metric is recorded goes through a
//! typed function that documents and enforces its label shape.

use metrics::{Counter, Gauge};

/// The result of a JSON-RPC request, as recorded by
/// [`rpc_requests_total`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcRequestResult {
    /// The server returned a successful JSON-RPC response.
    Success,
    /// The transport failed, the server returned a JSON-RPC error, or no
    /// matching response was returned.
    Failure,
}

impl RpcRequestResult {
    fn label(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// Number of JSON-RPC requests made through the shared provider, by `method`
/// and `result`.
pub fn rpc_requests_total(method: &str, result: RpcRequestResult) -> Counter {
    let result = result.label();
    metrics::counter!(
        "safenet_core_rpc_requests_total",
        "method" => method.to_owned(),
        "result" => result,
    )
}

/// The point in the chain-processing lifecycle represented by the cursor
/// gauges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingStatus {
    /// The update was received from the chain watcher.
    Seen,
    /// The update was successfully applied to the state machine.
    Processed,
}

impl ProcessingStatus {
    /// Returns all variants for the processing status.
    pub fn variants() -> impl Iterator<Item = Self> {
        [Self::Seen, Self::Processed].into_iter()
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Seen => "seen",
            Self::Processed => "processed",
        }
    }
}

/// The block-number component of the chain-processing cursor, by `status`.
pub fn block_number(status: ProcessingStatus) -> Gauge {
    let status = status.label();
    metrics::gauge!("safenet_core_block_number", "status" => status)
}

/// Number of live blocks invalidated by chain reorgs.
pub fn uncled_blocks_total() -> Counter {
    metrics::counter!("safenet_core_uncled_blocks_total")
}
