//! The sentinel's single effect: requesting a proposed transaction's verdict
//! from the sentinel engine. Emitted by
//! `SentinelTransition::handle_oracle_transaction_proposed` and consumed by
//! `SentinelTransition::apply_transition`'s `Message::Resume` arm.

use crate::{
    bindings::consensus::SafeTransaction,
    engine::{CheckOutcome, EngineClient},
    verdicts::VerdictStore,
};
use alloy::{eips::BlockNumberOrTag, primitives::B256};
use safenet_core::{effects::EffectHandler, index::BlockStatus};
use std::time::Duration;

/// An impure operation the sentinel's state transition asks the [`Handler`]
/// to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Defer the approve/deny decision for `request_id` (a proposed
    /// `transaction` on `safe`) to the configured sentinel engine.
    /// `proposal_timestamp` is the timestamp of the consensus-chain block the
    /// transaction was proposed in, if the node reported it.
    ///
    /// Performing this again for the same `request_id` (e.g. when a restart or
    /// reorg replays the proposal) resumes with the verdict recorded the first
    /// time rather than asking the engine again, since that is the verdict any
    /// commitment was built from. `block` is the number of the consensus-chain
    /// block the transaction was proposed in; it only decides how long that
    /// verdict is retained and is not sent to the engine.
    EngineCheck {
        request_id: B256,
        transaction: SafeTransaction,
        proposal_timestamp: Option<u64>,
        block: u64,
    },
}

/// The result of performing an [`Effect`], resumed into the state machine.
#[derive(Debug, Clone)]
pub enum Resume {
    /// Resume with [`Effect::EngineCheck`]'s outcome for `request_id`.
    EngineCheckResult {
        request_id: B256,
        outcome: CheckOutcome,
    },
}

/// Performs the sentinel's [`Effect`]s by asking the configured sentinel
/// engine for a verdict, recording it in the [`VerdictStore`].
pub struct Handler {
    engine: EngineClient,
    engine_timeout: Duration,
    verdicts: VerdictStore,
}

impl Handler {
    pub fn new(engine: EngineClient, engine_timeout: Duration, verdicts: VerdictStore) -> Self {
        Self {
            engine,
            engine_timeout,
            verdicts,
        }
    }

    /// Returns the verdict for `request_id`: the recorded one if there is one,
    /// otherwise a fresh one from the engine, recorded before it is returned.
    ///
    /// Any storage failure resolves to [`CheckOutcome::Unknown`]: without the
    /// store it is impossible to tell whether a verdict was already committed
    /// to, and a verdict that isn't recorded can't be reproduced for the
    /// reveal, so neither may be voted on.
    async fn engine_check(
        &self,
        request_id: B256,
        transaction: SafeTransaction,
        proposal_timestamp: Option<u64>,
        block: u64,
    ) -> CheckOutcome {
        match self.verdicts.get(request_id, block).await {
            Ok(Some(outcome)) => {
                tracing::debug!(%request_id, ?outcome, "reusing recorded engine verdict");
                return outcome;
            }
            Ok(None) => {}
            Err(err) => {
                tracing::error!(
                    %request_id,
                    ?err,
                    "failed to read recorded engine verdict; dropping the request unanswered"
                );
                return CheckOutcome::Unknown;
            }
        }

        // The sentinel only follows the consensus chain, so it has no block of
        // its own on the chain `transaction` executes on: defer to the
        // engine's view of that chain's latest block.
        let mut check = self
            .engine
            .security_check(BlockNumberOrTag::Latest, &transaction)
            .request_id(request_id)
            .timeout(self.engine_timeout);
        if let Some(timestamp) = proposal_timestamp {
            check = check.proposal_timestamp(timestamp);
        }
        let outcome = check.execute().await;
        // A concurrent check for the same request may have recorded its
        // verdict first, in which case that one is returned instead.
        match self.verdicts.record(request_id, block, outcome).await {
            Ok(outcome) => outcome,
            Err(err) => {
                tracing::error!(
                    %request_id,
                    ?err,
                    "failed to record engine verdict; dropping the request unanswered"
                );
                CheckOutcome::Unknown
            }
        }
    }
}

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&self, effect: Effect) -> Resume {
        match effect {
            Effect::EngineCheck {
                request_id,
                transaction,
                proposal_timestamp,
                block,
            } => Resume::EngineCheckResult {
                request_id,
                outcome: self
                    .engine_check(request_id, transaction, proposal_timestamp, block)
                    .await,
            },
        }
    }

    async fn housekeeping(&self, status: BlockStatus) {
        // Only verdicts for proposals below the snapshot boundary are
        // removed, so a proposal the state machine can still replay always
        // finds the verdict it was committed to.
        match self.verdicts.prune(status.safe).await {
            Ok(pruned) => {
                if pruned > 0 {
                    tracing::debug!(
                        block = status.latest,
                        safe = status.safe,
                        pruned,
                        "pruned engine verdicts"
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    block = status.latest,
                    safe = status.safe,
                    ?err,
                    "failed to prune engine verdicts"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::RuleId;
    use alloy::primitives::Address;
    use safenet_core::utils;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    const SAFE: Address = Address::new([1u8; 20]);
    const REQUEST_ID: B256 = B256::repeat_byte(0x11);

    /// Serves `bodies` in order, one per request, after which the engine goes
    /// away and every further request fails.
    async fn engine(bodies: &[&'static str]) -> EngineClient {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let bodies = bodies.to_vec();
        tokio::spawn(async move {
            for body in bodies {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        EngineClient::new(url).unwrap()
    }

    async fn handler(bodies: &[&'static str]) -> Handler {
        let pool = utils::connect_sqlite("sqlite::memory:".parse().unwrap())
            .await
            .unwrap();
        Handler::new(
            engine(bodies).await,
            Duration::from_secs(1),
            VerdictStore::new(pool).await.unwrap(),
        )
    }

    async fn check(handler: &Handler) -> CheckOutcome {
        let Resume::EngineCheckResult {
            request_id,
            outcome,
        } = handler
            .perform_effect(Effect::EngineCheck {
                request_id: REQUEST_ID,
                transaction: SafeTransaction {
                    safe: SAFE,
                    ..Default::default()
                },
                proposal_timestamp: Some(1_700_000_000),
                block: 1,
            })
            .await;
        assert_eq!(request_id, REQUEST_ID);
        outcome
    }

    #[tokio::test]
    async fn resumes_with_the_engine_outcome() {
        let handler = handler(&[r#"{"verdict":"secure"}"#]).await;

        assert_eq!(check(&handler).await, CheckOutcome::Approved);
    }

    #[tokio::test]
    async fn replayed_check_resumes_with_the_first_verdict() {
        let handler = handler(&[
            r#"{"verdict":"insecure","rule":"R-2.1"}"#,
            r#"{"verdict":"insecure","rule":"R-3.4"}"#,
        ])
        .await;
        let denied = CheckOutcome::Denied(RuleId::new(2, 1));

        assert_eq!(check(&handler).await, denied);
        // The engine now answers differently...
        assert_eq!(check(&handler).await, denied);
        // ...or not at all.
        assert_eq!(check(&handler).await, denied);
    }

    #[tokio::test]
    async fn check_without_a_verdict_is_asked_again() {
        let handler = handler(&[r#"{"verdict":"abstain"}"#, r#"{"verdict":"secure"}"#]).await;

        assert_eq!(check(&handler).await, CheckOutcome::Unknown);
        assert_eq!(check(&handler).await, CheckOutcome::Approved);
    }
}
