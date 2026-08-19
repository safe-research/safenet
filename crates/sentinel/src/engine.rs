//! Client for the sentinel engine used to check transactions that were not
//! conclusively handled by the sentinel's local checks.

use crate::{
    bindings::consensus::SafeTransaction,
    checker::{CheckOutcome, Checker},
};
use alloy::primitives::B256;
use reqwest::RequestBuilder;
use safe_tx::rule::RuleId;
use serde::{
    Deserialize, Serialize, Serializer,
    ser::{self, SerializeStruct as _},
};
use std::time::Duration;
use tracing::{Span, field};
use url::Url;

struct Request<'a> {
    transaction: &'a SafeTransaction,
}

impl<'a> Serialize for Request<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let transaction = safe_tx::SafeTransaction::try_from(self.transaction.clone())
            .map_err(ser::Error::custom)?;
        let mut request = serializer.serialize_struct("Request", 1)?;
        request.serialize_field("transaction", &transaction)?;
        request.end()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase", tag = "verdict")]
enum Response {
    Secure,
    Insecure { rule: RuleId },
    Abstain,
}

/// Posts a proposed transaction to an operator-configured sentinel engine and
/// parses its verdict.
///
/// A failed request is *not* treated as approval or denial: an unreachable
/// or malfunctioning sentinel engine isn't evidence about the transaction
/// either way, so the caller is expected to drop the request rather than
/// vote on it (see the `TODO` in `crate::effect`).
pub struct EngineClient {
    endpoint: Url,
    client: reqwest::Client,
}

/// An error constructing an [`EngineClient`].
#[derive(Debug, thiserror::Error)]
#[error("sentinel engine URL cannot be used as a base URL")]
pub struct InvalidBaseUrl;

impl EngineClient {
    /// Creates a client for the engine at `base_url`.
    pub fn new(mut base_url: Url) -> Result<Self, InvalidBaseUrl> {
        base_url
            .path_segments_mut()
            .map_err(|_| InvalidBaseUrl)?
            .pop_if_empty()
            .extend(["v1", "security-check"]);

        Ok(Self {
            endpoint: base_url,
            client: reqwest::Client::new(),
        })
    }

    /// Requests a verdict for `transaction`, correlating the HTTP request with
    /// the onchain request through the `x-request-id` header.
    ///
    /// An invalid transaction, transport failure, timeout, non-success status,
    /// or invalid response is not evidence for either vote and therefore
    /// resolves to [`CheckOutcome::Unknown`].
    pub fn security_check(&self, transaction: &SafeTransaction) -> SecurityCheck {
        let span = tracing::info_span!(
            "security_check",
            safe = %transaction.safe,
            request_id = field::Empty,
        );
        let request = self
            .client
            .post(self.endpoint.clone())
            .json(&Request { transaction });

        SecurityCheck { span, request }
    }
}

/// A security check.
pub struct SecurityCheck {
    span: Span,
    request: RequestBuilder,
}

impl SecurityCheck {
    /// Configure the request ID for the security check.
    #[expect(
        dead_code,
        reason = "the sentinel is not yet wired to propagate request ID to the engine"
    )]
    pub fn request_id(mut self, request_id: B256) -> Self {
        self.span.record("request_id", field::display(request_id));
        self.request = self.request.header("X-Request-ID", request_id.to_string());
        self
    }

    /// Configure the timeout for the security check.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.request = self.request.timeout(timeout).header(
            "X-Request-Timeout",
            u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
        );
        self
    }

    /// Executes the security check.
    pub async fn execute(self) -> CheckOutcome {
        let _entered = self.span.enter();
        async move {
            let response = self
                .request
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let outcome = match response {
                Response::Secure => CheckOutcome::Approved,
                Response::Insecure { rule } => CheckOutcome::Denied(rule),
                Response::Abstain => CheckOutcome::Unknown,
            };
            Ok(outcome)
        }
        .await
        .unwrap_or_else(|err: reqwest::Error| {
            tracing::error!(
                %err,
                "sentinel engine request failed; dropping the request unanswered",
            );

            // A failed request is *not* treated as approval or denial: an
            // unreachable or malfunctioning sentinel engine isn't evidence
            // about the  transaction either way, so it resolves to
            // [`CheckOutcome::Unknown`], deferring to whatever checker runs
            // next (or, if this is the last one in the chain, dropping the
            // request rather than voting on it).
            CheckOutcome::Unknown
        })
    }
}

#[async_trait::async_trait]
impl Checker for EngineClient {
    async fn check(&self, transaction: &SafeTransaction) -> CheckOutcome {
        self.security_check(transaction)
            // TODO: Invoke `security_check` directly from the effect handler once
            // it can supply the real request ID and remaining response time.
            .timeout(Duration::from_secs(10))
            .execute()
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    /// Serves `body` (with `status`, e.g. `"200 OK"`) to the single request
    /// a test sends, on a one-shot localhost listener.
    async fn respond_once(status: &'static str, body: &'static str) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
        });
        url
    }

    #[tokio::test]
    async fn approves_when_the_endpoint_approves() {
        let url = respond_once("200 OK", r#"{"verdict":"secure"}"#).await;
        let checker = EngineClient::new(url).unwrap();
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn denies_with_the_cited_rule() {
        let url = respond_once("200 OK", r#"{"verdict":"insecure","rule":"R-4.6"}"#).await;
        let checker = EngineClient::new(url).unwrap();
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Denied(RuleId::R4_6KnownMaliciousTarget)
        );
    }

    #[tokio::test]
    async fn fails_on_an_unrecognized_rule_code() {
        let url = respond_once(
            "200 OK",
            r#"{"verdict":"insecure","rule":"not-a-real-rule"}"#,
        )
        .await;
        let checker = EngineClient::new(url).unwrap();
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn abstains_when_the_endpoint_abstains() {
        let url = respond_once("200 OK", r#"{"verdict":"abstain"}"#).await;
        let checker = EngineClient::new(url).unwrap();
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn fails_when_the_endpoint_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        drop(listener);

        let checker = EngineClient::new(url).unwrap();
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Unknown
        );
    }
}
