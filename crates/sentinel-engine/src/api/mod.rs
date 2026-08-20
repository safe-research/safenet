//! HTTP API for the sentinel engine.

mod extractors;

use self::extractors::{RequestId, RequestTimeout};
use crate::engine::{SafeTransaction, SentinelEngine, Verdict};
use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{Instrument as _, field};

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
        .layer(TraceLayer::new_for_http())
}

async fn security_check(
    State(engine): State<Arc<SentinelEngine>>,
    RequestId(request_id): RequestId,
    RequestTimeout(timeout): RequestTimeout,
    Json(request): Json<CheckRequest>,
) -> Json<Verdict> {
    let span = tracing::info_span!(
         "security_check",
         safe = %request.transaction.safe,
         request_id = field::Empty,
    );
    if let Some(request_id) = request_id {
        span.record("request_id", field::display(request_id));
    }

    // TODO: Pass these parameters to the engine once it supports request
    // lifecycle context.
    let _ = timeout;

    let verdict = engine
        .security_check(request.transaction)
        .instrument(span)
        .await;
    Json(verdict)
}
