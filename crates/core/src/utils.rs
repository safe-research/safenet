//! Miscellaneous utilities shared across Safenet services that don't belong
//! to any single component.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tokio::signal::unix;

/// Creates a signal for supporting graceful shutdown in services.
///
/// Intercepts both `SIGTERM` (usually sent by container runtimes) and `SIGINT`
/// (sent by `Ctrl-C`).
pub async fn shutdown_signal() {
    let sigterm = async {
        unix::signal(unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await
    };
    let sigint = async {
        unix::signal(unix::SignalKind::interrupt())
            .unwrap()
            .recv()
            .await;
    };
    tokio::pin!(sigterm);
    tokio::pin!(sigint);
    tokio::select! {
        _ = sigterm => {},
        _ = sigint => {},
    };
}

/// Connects a `SqlitePool`, for use as [`Driver::new`](crate::Driver::new)'s
/// `pool` argument (or anywhere else a service needs a SQLite pool).
///
/// This disables sqlx's default connection recycling (`idle_timeout` and
/// `max_lifetime`), which is critical for a `sqlite::memory:` database: an
/// in-memory database only exists for as long as at least one connection to
/// it is open, but sqlx's pool otherwise periodically closes and reopens even
/// a lone remaining connection to enforce those limits. The instant that
/// happens, the (named, potentially shared-cache) in-memory database is
/// destroyed, and the next query transparently opens a fresh, empty one in
/// its place — surfacing as spurious "no such table" errors with no apparent
/// cause. A file-backed database has no such failure mode, but there is no
/// upside to recycling a long-lived, single-process SQLite connection either,
/// so this is disabled unconditionally rather than only for `:memory:`.
///
/// Note that `min_connections` does not help here: sqlx's reaper still closes
/// and reopens a connection to enforce `idle_timeout`/`max_lifetime` even
/// while maintaining a minimum pool size, so both must be disabled outright.
pub async fn connect_sqlite(options: SqliteConnectOptions) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await
}
