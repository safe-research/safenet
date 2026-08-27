//! Client for the sentinel engine used to check proposed transactions.

use crate::{bindings::consensus::SafeTransaction, metrics::EngineCheckVerdict};
use alloy::primitives::B256;
use reqwest::RequestBuilder;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{borrow::Cow, fmt, time::Duration};
use tracing::{Instrument as _, Span, field};
use url::Url;

/// The result of checking a proposed transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// The transaction should receive an approving vote.
    Approved,
    /// The transaction should receive a denying vote citing this rule.
    Denied(RuleId),
    /// There is no trustworthy verdict, so the request is dropped unanswered
    /// rather than guessed at.
    Unknown,
}

/// A Safenet Arbitration Charter rule citation.
///
/// The sentinel deliberately treats this as an open-ended identifier rather
/// than an enum: the connected engine may implement rules added after this
/// sentinel version was released.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleId(u32, u32);

impl RuleId {
    /// Creates the rule `R-{section}.{rule}`.
    pub const fn new(section: u32, rule: u32) -> Self {
        Self(section, rule)
    }

    fn parse(code: &str) -> Option<Self> {
        let (section, rule) = code.strip_prefix("R-")?.split_once('.')?;
        let section = section.parse().ok()?;
        let rule = rule.parse().ok()?;
        Some(Self::new(section, rule))
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "R-{}.{}", self.0, self.1)
    }
}

impl Serialize for RuleId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RuleId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code = Cow::<str>::deserialize(deserializer)?;
        Self::parse(&code)
            .ok_or_else(|| de::Error::custom(format_args!("invalid rule ID `{code}`")))
    }
}

#[derive(Serialize)]
struct Request<'a> {
    #[serde(with = "alloy::serde::quantity")]
    block: u64,
    transaction: &'a SafeTransaction,
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
/// either way, so the caller drops the request rather than voting on it.
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
    /// the onchain request through the `x-request-id` header. `block` is the
    /// block number the sentinel considers current, sent along so the engine
    /// can use it as a reference point when reading chain state.
    ///
    /// An invalid transaction, transport failure, timeout, non-success status,
    /// or invalid response is not evidence for either vote and therefore
    /// resolves to [`CheckOutcome::Unknown`].
    pub fn security_check(&self, block: u64, transaction: &SafeTransaction) -> SecurityCheck {
        let span = tracing::info_span!(
            "security_check",
            safe = %transaction.safe,
            request_id = field::Empty,
        );
        let request = self
            .client
            .post(self.endpoint.clone())
            .json(&Request { block, transaction });

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
    pub fn request_id(mut self, request_id: B256) -> Self {
        self.span.record("request_id", field::display(request_id));
        self.request = self.request.header("x-request-id", request_id.to_string());
        self
    }

    /// Configure the timeout for the security check.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.request = self.request.timeout(timeout).header(
            "x-request-timeout",
            u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
        );
        self
    }

    /// Executes the security check.
    pub async fn execute(self) -> CheckOutcome {
        let Self { span, request } = self;
        async move {
            let result: Result<Response, reqwest::Error> =
                async { request.send().await?.error_for_status()?.json().await }.await;

            let (outcome, verdict) = match result {
                Ok(Response::Secure) => (CheckOutcome::Approved, EngineCheckVerdict::Secure),
                Ok(Response::Insecure { rule }) => {
                    (CheckOutcome::Denied(rule), EngineCheckVerdict::Insecure)
                }
                Ok(Response::Abstain) => (CheckOutcome::Unknown, EngineCheckVerdict::Abstain),
                // A failed request is *not* treated as approval or denial: an
                // unreachable or malfunctioning sentinel engine isn't evidence
                // about the transaction either way, so it resolves to
                // [`CheckOutcome::Unknown`], dropping the request rather than
                // voting on it.
                Err(err) => {
                    tracing::error!(
                        %err,
                        "sentinel engine request failed; dropping the request unanswered",
                    );
                    (CheckOutcome::Unknown, EngineCheckVerdict::Error)
                }
            };
            crate::metrics::engine_check_verdicts_total(verdict).increment(1);
            outcome
        }
        .instrument(span)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
        time,
    };

    /// Serves `body` (with `status`, e.g. `"200 OK"`) after a `delay` to
    /// the single request a test sends, on a one-shot localhost listener.
    ///
    /// Returns a channel for receiving the request body.
    async fn respond_once_ex(
        status: &'static str,
        body: &'static str,
        delay: Duration,
    ) -> (Url, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let size = stream.read(&mut buf).await.unwrap();
            let request = str::from_utf8(&buf[..size]).unwrap().to_owned();
            let _ = sender.send(request);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            tokio::time::sleep(delay).await;
            let _ = stream.write_all(response.as_bytes()).await;
        });
        (url, receiver)
    }

    /// Serves `body` (with `status`, e.g. `"200 OK"`) to the single request
    /// a test sends, on a one-shot localhost listener.
    async fn respond_once(status: &'static str, body: &'static str) -> Url {
        let (url, _) = respond_once_ex(status, body, Duration::ZERO).await;
        url
    }

    #[tokio::test]
    async fn forwards_request_headers() {
        let (url, request) =
            respond_once_ex("200 OK", r#"{"verdict":"secure"}"#, Duration::ZERO).await;
        let engine = EngineClient::new(url).unwrap();

        let _ = engine
            .security_check(1, &SafeTransaction::default())
            .request_id(B256::repeat_byte(0x42))
            .timeout(Duration::from_millis(1337))
            .execute()
            .await;
        let request = request.await.unwrap();

        for header in [
            "x-request-id: 0x4242424242424242424242424242424242424242424242424242424242424242",
            "x-request-timeout: 1337",
        ] {
            assert!(
                request.contains(&format!("{header}\r\n")),
                "request does not container header '{header}':\n---\n{request}\n---"
            )
        }
    }

    #[tokio::test]
    async fn approves_when_the_endpoint_approves() {
        let url = respond_once("200 OK", r#"{"verdict":"secure"}"#).await;
        let engine = EngineClient::new(url).unwrap();
        assert_eq!(
            engine
                .security_check(1, &SafeTransaction::default())
                .execute()
                .await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn denies_with_the_cited_rule() {
        let url = respond_once("200 OK", r#"{"verdict":"insecure","rule":"R-4.6"}"#).await;
        let engine = EngineClient::new(url).unwrap();
        assert_eq!(
            engine
                .security_check(1, &SafeTransaction::default())
                .execute()
                .await,
            CheckOutcome::Denied(RuleId::new(4, 6))
        );
    }

    #[tokio::test]
    async fn accepts_a_rule_not_known_to_the_sentinel() {
        let url = respond_once("200 OK", r#"{"verdict":"insecure","rule":"R-42.1337"}"#).await;
        let engine = EngineClient::new(url).unwrap();
        assert_eq!(
            engine
                .security_check(1, &SafeTransaction::default())
                .execute()
                .await,
            CheckOutcome::Denied(RuleId::new(42, 1337))
        );
    }

    #[tokio::test]
    async fn fails_on_a_malformed_rule_code() {
        let url = respond_once(
            "200 OK",
            r#"{"verdict":"insecure","rule":"not-a-real-rule"}"#,
        )
        .await;
        let engine = EngineClient::new(url).unwrap();
        assert_eq!(
            engine
                .security_check(1, &SafeTransaction::default())
                .execute()
                .await,
            CheckOutcome::Unknown
        );
    }

    #[test]
    fn rule_id_json_roundtrip() {
        let rule = RuleId::new(42, 1337);
        let json = serde_json::to_string(&rule).unwrap();

        assert_eq!(json, r#""R-42.1337""#);
        assert_eq!(serde_json::from_str::<RuleId>(&json).unwrap(), rule);
    }

    #[tokio::test]
    async fn abstains_when_the_endpoint_abstains() {
        let url = respond_once("200 OK", r#"{"verdict":"abstain"}"#).await;
        let engine = EngineClient::new(url).unwrap();
        assert_eq!(
            engine
                .security_check(1, &SafeTransaction::default())
                .execute()
                .await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn fails_when_the_endpoint_returns_an_error() {
        let url = respond_once("503 Service Unavailable", r#"{"error":"unavailable"}"#).await;
        let engine = EngineClient::new(url).unwrap();
        assert_eq!(
            engine
                .security_check(1, &SafeTransaction::default())
                .execute()
                .await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn fails_when_the_endpoint_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        drop(listener);

        let engine = EngineClient::new(url).unwrap();
        assert_eq!(
            engine
                .security_check(1, &SafeTransaction::default())
                .execute()
                .await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fails_when_the_request_times_out() {
        let (url, _) =
            respond_once_ex("200 OK", r#"{"verdict":"secure"}"#, Duration::from_secs(60)).await;

        let engine = EngineClient::new(url).unwrap();
        let outcome = time::timeout(
            Duration::from_secs(50),
            engine
                .security_check(1, &SafeTransaction::default())
                .timeout(Duration::from_secs(1))
                .execute(),
        )
        .await
        .expect("the engine request timeout did not fire");

        assert_eq!(outcome, CheckOutcome::Unknown);
    }
}
