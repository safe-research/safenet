//! This crate's own Prometheus metrics.
//!
//! Each metric's name and its expected labels are defined together in one
//! accessor function here, so every place a metric is recorded goes through a
//! typed function that documents and enforces its label shape.

use metrics::Counter;

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
