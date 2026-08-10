//! The externally-pluggable check an operator can defer to for whatever
//! isn't implemented locally in [`crate::static_checker::StaticChecker`] —
//! dynamic lists, tracing/simulation, off-chain statistics, or anything else
//! a sentinel maintainer wants to run that doesn't belong in this crate.
//! [`RemoteChecker`] is this initial cut's only implementation: a plain
//! HTTPS POST issued inline, not a separate crate/service. Its
//! [`RemoteChecker`]'s [`Checker`] impl is already the seam to split
//! "trigger this endpoint, parse the response" along if that ever needs to
//! move out on its own.

use crate::{
    bindings::consensus::SafeTransaction,
    checker::{CheckOutcome, Checker},
};
use alloy::primitives::Address;
use safe_tx::rule::RuleId;
use serde::{Deserialize, Serialize};
use url::Url;

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
    /// resolves to [`CheckOutcome::Approved`] without a request, so the
    /// reference Sentinel works with just its local checks until an
    /// operator opts into a remote one.
    pub fn new(url: Option<Url>) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }

    async fn request(
        &self,
        url: &Url,
        transaction: &SafeTransaction,
    ) -> Result<CheckOutcome, reqwest::Error> {
        let response: Response = self
            .client
            .post(url.clone())
            .json(&Request {
                safe: transaction.safe,
                transaction,
            })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(
            match (
                response.approve,
                response
                    .rule
                    .as_deref()
                    .and_then(|rule| rule.parse::<RuleId>().ok()),
            ) {
                (true, _) => CheckOutcome::Approved,
                (false, Some(rule)) => CheckOutcome::Denied(rule),
                (false, _) => {
                    tracing::error!(safe = %transaction.safe, "remote check denied without a recognized rule code");
                    CheckOutcome::Unknown
                }
            },
        )
    }
}

#[async_trait::async_trait]
impl Checker for RemoteChecker {
    /// A failed request is *not* treated as approval or denial: an
    /// unreachable or malfunctioning remote check isn't evidence about the
    /// transaction either way, so it resolves to [`CheckOutcome::Unknown`],
    /// deferring to whatever checker runs next (or, if this is the last one
    /// in the chain, dropping the request rather than voting on it).
    async fn check(&self, transaction: &SafeTransaction) -> CheckOutcome {
        let Some(url) = &self.url else {
            return CheckOutcome::Approved;
        };
        self.request(url, transaction).await.unwrap_or_else(|err| {
            tracing::error!(%err, safe = %transaction.safe, "remote check request failed; dropping the request unanswered");
            CheckOutcome::Unknown
        })
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
    async fn approves_without_a_request_when_unconfigured() {
        let checker = RemoteChecker::new(None);
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn approves_when_the_endpoint_approves() {
        let url = respond_once("200 OK", r#"{"approve":true,"rule":null}"#).await;
        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn denies_with_the_cited_rule() {
        let url = respond_once("200 OK", r#"{"approve":false,"rule":"R-4.6"}"#).await;
        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Denied(RuleId::KNOWN_MALICIOUS_TARGET)
        );
    }

    #[tokio::test]
    async fn denies_with_an_unknown_well_formed_rule_code() {
        let url = respond_once("200 OK", r#"{"approve":false,"rule":"R-9.9"}"#).await;
        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Denied(RuleId::new(9, 9))
        );
    }

    #[tokio::test]
    async fn fails_on_a_malformed_rule_code() {
        let url = respond_once("200 OK", r#"{"approve":false,"rule":"not-a-real-rule"}"#).await;
        let checker = RemoteChecker::new(Some(url));
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

        let checker = RemoteChecker::new(Some(url));
        assert_eq!(
            checker.check(&SafeTransaction::default()).await,
            CheckOutcome::Unknown
        );
    }
}
