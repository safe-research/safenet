//! Observability helpers shared across Safenet services: a default `tracing`
//! logging setup and a Prometheus metrics exporter.

use serde::{Deserialize, Deserializer, de};
use std::{
    borrow::Cow,
    net::{Ipv4Addr, SocketAddr},
};
use tracing_subscriber::EnvFilter;

pub mod logging;
pub mod metrics;

/// Configuration for the observability module.
///
/// Deserializes from a configuration file and fills in defaults for any omitted
/// field, so a consumer only specifies the values it needs to override.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The `tracing` env-filter directive controlling log verbosity (see
    /// [`logging::init`]). Defaults to `info`.
    #[serde(deserialize_with = "deserialize_log_filter")]
    pub log_filter: EnvFilter,
    /// The address the Prometheus metrics HTTP listener binds to (see
    /// [`metrics::serve`]). Defaults to `127.0.0.1:0`, which picks an ephemeral
    /// port on the loopback interface.
    pub metrics_address: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_filter: EnvFilter::new("info"),
            metrics_address: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        }
    }
}

/// Error returned when initializing observability fails. Both logging and
/// metrics install process-global state, so the most common cause is that
/// [`init`] (or its components) was already called in this process.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    /// The global `tracing` subscriber could not be installed.
    #[error("failed to initialize logging: {0}")]
    Logging(#[from] tracing_subscriber::util::TryInitError),
    /// The global Prometheus recorder or scrape listener could not be installed.
    #[error("failed to initialize metrics: {0}")]
    Metrics(#[from] metrics_exporter_prometheus::BuildError),
}

/// Initializes logging and metrics from the given [`Config`].
///
/// This installs the global `tracing` subscriber (see [`logging::init`]) and
/// the global Prometheus recorder and scrape listener (see [`metrics::serve`]).
/// It can only succeed once per process; a later call returns an [`InitError`].
pub fn init(config: Config) -> Result<(), InitError> {
    logging::init(config.log_filter)?;
    metrics::serve(config.metrics_address)?;
    Ok(())
}

fn deserialize_log_filter<'de, D>(deserializer: D) -> Result<EnvFilter, D::Error>
where
    D: Deserializer<'de>,
{
    EnvFilter::try_new(Cow::deserialize(deserializer)?).map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let config = serde_json::from_str::<Config>("{}").unwrap();
        assert_eq!(
            config.log_filter.to_string(),
            Config::default().log_filter.to_string(),
        );
        assert_eq!(config.metrics_address, Config::default().metrics_address);
    }

    #[test]
    fn deserializes_overrides() {
        let config = serde_json::from_str::<Config>(
            r#"{
                "log_filter": "debug",
                "metrics_address": "0.0.0.0:9000"
            }"#,
        )
        .unwrap();
        assert_eq!(config.log_filter.to_string(), "debug");
        assert_eq!(
            config.metrics_address,
            SocketAddr::from(([0, 0, 0, 0], 9000)),
        );
    }
}
