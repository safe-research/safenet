//! Prometheus metrics exporter for Safenet services.

use metrics_exporter_prometheus::{BuildError, PrometheusBuilder};
use std::net::SocketAddr;

/// Installs the global Prometheus recorder and starts an HTTP listener that
/// scrapers can poll for metrics.
///
/// The listener serves Prometheus-formatted metrics on every path except
/// `/health`, which returns a plain `OK` for liveness probes. Individual
/// metrics are not registered here; services record them lazily through the
/// [`metrics`](https://docs.rs/metrics) facade and they appear automatically.
///
/// This installs a process-global recorder and so can only succeed once; later
/// calls return a [`BuildError`].
pub fn serve(addr: impl Into<SocketAddr>) -> Result<(), BuildError> {
    PrometheusBuilder::new()
        .with_http_listener(addr.into())
        .install()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::transports::http::reqwest::{self, StatusCode};
    use std::net::TcpListener;

    /// Issues a minimal HTTP/1.1 GET and returns the full raw response.
    async fn http_get(addr: SocketAddr, path: &str) -> (StatusCode, String) {
        let result = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        (result.status(), result.text().await.unwrap())
    }

    #[tokio::test]
    async fn serves_metrics_and_health() {
        // Reserve an ephemeral port, then hand it to the exporter. The listener
        // binds eagerly during `serve`, so it is ready once `serve` returns.
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        serve(addr).unwrap();

        metrics::counter!("safenet_core_test_total").increment(1);

        let (status, metrics) = http_get(addr, "/metrics").await;
        assert_eq!(status, StatusCode::OK);
        assert!(metrics.contains("safenet_core_test_total"));

        let (status, health) = http_get(addr, "/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(health, "OK");
    }
}
