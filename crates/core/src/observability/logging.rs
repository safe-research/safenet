//! Default `tracing` subscriber setup for Safenet services.

use std::io::IsTerminal as _;
use tracing_subscriber::{EnvFilter, fmt, prelude::*, util::TryInitError};

/// Initializes the global `tracing` subscriber with sensible defaults.
///
/// The `filter` controls logging verbosity. Output is human-readable when
/// stdout is a terminal and JSON-formatted otherwise, so logs are easy to read
/// during development and easy to aggregate in production.
///
/// This installs a process-global subscriber and so can only succeed once;
/// later calls return a [`TryInitError`].
pub fn init(filter: EnvFilter) -> Result<(), TryInitError> {
    let registry = tracing_subscriber::registry().with(filter);
    if std::io::stdout().is_terminal() {
        registry.with(fmt::layer()).try_init()
    } else {
        registry.with(fmt::layer().json()).try_init()
    }
}
