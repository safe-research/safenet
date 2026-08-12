use crate::engine::{SentinelEngine, Verdict};
use axum::{
    Json, Router,
    extract::{Request, State},
    routing::post,
};
use safe_tx::SafeTransaction;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{self, AtomicUsize},
};
use tower_http::trace::TraceLayer;

/// A request to verify a proposed Safe transaction.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRequest {
    /// The transaction to verify.
    pub transaction: SafeTransaction,
}

/// Constructs the transaction-checking API.
pub fn router(engine: SentinelEngine) -> Router {
    let engine = Arc::new(engine);
    Router::new()
        .route("/v1/security-check", post(security_check))
        .with_state(engine)
        .layer(TraceLayer::new_for_http().make_span_with(request_id))
}

async fn security_check(
    State(engine): State<Arc<SentinelEngine>>,
    Json(request): Json<CheckRequest>,
) -> Json<Verdict> {
    let verdict = engine.security_check(request.transaction).await;
    Json(verdict)
}

fn request_id(request: &Request) -> tracing::Span {
    let request_id = if let Some(header) = request.headers().get("x-request-id") {
        let raw = header.as_bytes();
        // Only include the first 128 bytes of the request ID to prevent very
        // long IDs making the logs hard to read.
        let trimmed = &raw[..raw.len().min(128)];
        String::from_utf8_lossy(trimmed).into_owned()
    } else {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        format!("{}", COUNTER.fetch_add(1, atomic::Ordering::Relaxed))
    };

    let span = tracing::info_span!("http_request", request_id = %request_id);
    {
        let _span = span.enter();
        tracing::trace!(
            uri = %request.uri(),
            method = %request.method(),
            "HTTP request");
    }

    span
}
