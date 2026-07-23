//! The externally-pluggable check an operator can defer to for whatever
//! isn't implemented locally in [`crate::static_checker::StaticChecker`] —
//! dynamic lists, tracing/simulation, off-chain statistics, or anything else
//! a sentinel maintainer wants to run that doesn't belong in this crate.
//! [`RemoteChecker`] is this initial cut's only implementation: a plain
//! HTTPS POST issued inline, not a separate crate/service. Its
//! [`RemoteChecker::check`] method is already the seam to split "trigger
//! this endpoint, parse the response" along if that ever needs to move out
//! on its own, or to extract a trait behind if a second implementation
//! shows up.

use alloy::primitives::Address;
use safe_tx::{rule::RuleId, types::SafeTransaction};
use serde::{Deserialize, Serialize};
use url::Url;

/// The outcome of a [`RemoteChecker::check`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteCheckOutcome {
    /// No configured remote check denied the transaction (or none is
    /// configured at all).
    Approved,
    /// The remote check denied the transaction, citing this rule.
    Denied(RuleId),
    /// The endpoint couldn't be reached, or answered with something that
    /// can't be trusted (a non-2xx status, a malformed body, or a `rule`
    /// code this Sentinel doesn't recognize). Callers must not guess at
    /// approve/deny for this outcome — see the caveat on [`RemoteChecker`].
    Failed,
}

#[derive(Serialize)]
struct Request<'a> {
    safe: Address,
    transaction: &'a SafeTransaction,
}

#[derive(Deserialize)]
struct Response {
    approve: bool,
    rule: Option<String>,
}

/// Posts a proposed transaction to an operator-configured endpoint and
/// parses its verdict.
///
/// A failed request is *not* treated as approval or denial: an unreachable
/// or malfunctioning remote check isn't evidence about the transaction
/// either way, so the caller is expected to drop the request rather than
/// vote on it (see the `TODO` in `crate::effect`).
pub struct RemoteChecker {
    url: Option<Url>,
    client: reqwest::Client,
}

impl RemoteChecker {
    /// `url: None` means no remote check is configured; every call then
    /// resolves to [`RemoteCheckOutcome::Approved`] without a request, so
    /// the reference Sentinel works with just its local checks until an
    /// operator opts into a remote one.
    pub fn new(url: Option<Url>) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn check(&self, safe: Address, transaction: &SafeTransaction) -> RemoteCheckOutcome {
        let Some(url) = &self.url else {
            return RemoteCheckOutcome::Approved;
        };
        self.request(url, safe, transaction).await.unwrap_or_else(|err| {
            tracing::error!(%err, %safe, "remote check request failed; dropping the request unanswered");
            RemoteCheckOutcome::Failed
        })
    }

    async fn request(
        &self,
        url: &Url,
        safe: Address,
        transaction: &SafeTransaction,
    ) -> Result<RemoteCheckOutcome, reqwest::Error> {
        let response: Response = self
            .client
            .post(url.clone())
            .json(&Request { safe, transaction })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(
            match (
                response.approve,
                response.rule.as_deref().map(RuleId::from_code),
            ) {
                (true, _) => RemoteCheckOutcome::Approved,
                (false, Some(Some(rule))) => RemoteCheckOutcome::Denied(rule),
                (false, _) => {
                    tracing::error!(%safe, "remote check denied without a recognized rule code");
                    RemoteCheckOutcome::Failed
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    const SAFE: Address = address!("1111111111111111111111111111111111111111");

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
    async fn approves_without_a_request_when_unconfigured() {
        let checker = RemoteChecker::new(None);
        assert_eq!(
            checker.check(SAFE, &SafeTransaction::default()).await,
            RemoteCheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn approves_when_the_endpoint_approves() {
        let url = respond_once("200 OK", r#"{"approve":true,"rule":null}"#).await;
        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(SAFE, &SafeTransaction::default()).await,
            RemoteCheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn denies_with_the_cited_rule() {
        let url = respond_once("200 OK", r#"{"approve":false,"rule":"R-4.6"}"#).await;
        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(SAFE, &SafeTransaction::default()).await,
            RemoteCheckOutcome::Denied(RuleId::R4_6KnownMaliciousTarget)
        );
    }

    #[tokio::test]
    async fn fails_on_an_unrecognized_rule_code() {
        let url = respond_once("200 OK", r#"{"approve":false,"rule":"not-a-real-rule"}"#).await;
        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(SAFE, &SafeTransaction::default()).await,
            RemoteCheckOutcome::Failed
        );
    }

    #[tokio::test]
    async fn fails_when_the_endpoint_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        drop(listener);

        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(SAFE, &SafeTransaction::default()).await,
            RemoteCheckOutcome::Failed
        );
    }
}
