//! Prometheus metrics exporter for Safenet services.

use metrics_exporter_prometheus::{BuildError, PrometheusBuilder};
use std::net::{SocketAddr, TcpListener};

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
pub fn serve(addr: impl Into<SocketAddr>) -> Result<SocketAddr, BuildError> {
    let addr = addr.into();

    // The prometheus builder does not expose enough information to read the
    // socket address that was used for the HTTP listener. This matters in the
    // case where we bind to `:0` (where the OS assigns a random port). To work
    // around this, get assigned a random port by the OS and use that for the
    // listener. There is a race condition where the port _may_ become
    // unavailable after being initially assigned, so retry a few times (in
    // practice, this should work the very first time).
    if addr.port() == 0 {
        let try_install = || {
            let addr = TcpListener::bind(addr)
                .and_then(|listener| listener.local_addr())
                .map_err(|err| BuildError::FailedToCreateHTTPListener(err.to_string()))?;
            PrometheusBuilder::new()
                .with_http_listener(addr)
                .install()?;
            Ok(addr)
        };
        for _ in 0..2 {
            if let Ok(addr) = try_install() {
                return Ok(addr);
            }
        }
        try_install()
    } else {
        PrometheusBuilder::new()
            .with_http_listener(addr)
            .install()?;
        Ok(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::transports::http::reqwest::{self, StatusCode};
    use std::net::Ipv4Addr;

    /// Issues a minimal HTTP/1.1 GET and returns the full raw response.
    async fn http_get(addr: SocketAddr, path: &str) -> (StatusCode, String) {
        let result = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        (result.status(), result.text().await.unwrap())
    }

    #[tokio::test]
    async fn serves_metrics_and_health() {
        let addr = serve((Ipv4Addr::LOCALHOST, 0)).unwrap();

        metrics::counter!("safenet_core_test_total").increment(1);

        let (status, metrics) = http_get(addr, "/metrics").await;
        assert_eq!(status, StatusCode::OK);
        assert!(metrics.contains("safenet_core_test_total"));

        let (status, health) = http_get(addr, "/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(health, "OK");
    }
}
