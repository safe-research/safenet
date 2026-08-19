//! The sentinel's single effect: deferring a proposed transaction's
//! remaining approve/deny decision to [`Handler`]'s checker chain once the
//! local, synchronous checks in [`crate::static_checker::StaticChecker`]
//! have passed. Emitted by
//! `SentinelTransition::handle_oracle_transaction_proposed` and consumed by
//! `SentinelTransition::apply_transition`'s `Message::Resume` arm.

use crate::{
    bindings::consensus::SafeTransaction,
    checker::{CheckOutcome, Checker},
    engine::EngineClient,
};
use alloy::primitives::B256;
use safenet_core::effects::EffectHandler;
use std::time::Duration;

/// An impure operation the sentinel's state transition asks the [`Handler`]
/// to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Defer the approve/deny decision for `request_id` (a proposed
    /// `transaction` on `safe`) to the configured checker chain.
    DynamicCheck {
        request_id: B256,
        transaction: SafeTransaction,
    },
}

/// The result of performing an [`Effect`], resumed into the state machine.
#[derive(Debug, Clone)]
pub enum Resume {
    /// Resume with [`Effect::DynamicCheck`]'s outcome for `request_id`.
    DynamicCheckResult {
        request_id: B256,
        outcome: CheckOutcome,
    },
}

/// Performs the sentinel's [`Effect`]s by running its local [`Checker`] chain
/// in order, stopping at the first non-[`CheckOutcome::Unknown`] result. If
/// every local checker resolves to `Unknown`, the sentinel engine is the final
/// fallback — see `crate::checker` for why an engine failure remains `Unknown`
/// rather than being guessed at.
///
/// This is an initial cut of the "array of checkers" shape (see
/// `crate::checker`'s module docs); the fixed construction order below is
/// what encodes today's precedence (the built-in CoW check ahead of
/// address-poisoning, ahead of the sentinel engine).
pub struct Handler {
    checkers: Vec<Box<dyn Checker>>,
    engine: EngineClient,
    engine_timeout: Duration,
}

impl Handler {
    pub fn new(
        checkers: Vec<Box<dyn Checker>>,
        engine: EngineClient,
        engine_timeout: Duration,
    ) -> Self {
        Self {
            checkers,
            engine,
            engine_timeout,
        }
    }
}

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&self, effect: Effect) -> Resume {
        match effect {
            Effect::DynamicCheck {
                request_id,
                transaction,
            } => {
                let mut outcome = CheckOutcome::Unknown;
                for checker in &self.checkers {
                    outcome = checker.check(&transaction).await;
                    if outcome != CheckOutcome::Unknown {
                        break;
                    }
                }
                if outcome == CheckOutcome::Unknown {
                    outcome = self
                        .engine
                        .security_check(&transaction)
                        .request_id(request_id)
                        .timeout(self.engine_timeout)
                        .execute()
                        .await;
                }
                Resume::DynamicCheckResult {
                    request_id,
                    outcome,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    const SAFE: Address = Address::new([1u8; 20]);
    const REQUEST_ID: B256 = B256::repeat_byte(0x11);

    struct StubChecker(CheckOutcome);

    #[async_trait::async_trait]
    impl Checker for StubChecker {
        async fn check(&self, _: &SafeTransaction) -> CheckOutcome {
            self.0
        }
    }

    async fn engine(body: &'static str) -> EngineClient {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        EngineClient::new(url).unwrap()
    }

    #[tokio::test]
    async fn resumes_with_the_checker_s_outcome() {
        let engine = EngineClient::new("http://127.0.0.1:1".parse().unwrap()).unwrap();
        let handler = Handler::new(
            vec![
                Box::new(StubChecker(CheckOutcome::Unknown)),
                Box::new(StubChecker(CheckOutcome::Approved)),
            ],
            engine,
            Duration::from_secs(1),
        );

        let resume = handler
            .perform_effect(Effect::DynamicCheck {
                request_id: REQUEST_ID,
                transaction: SafeTransaction {
                    safe: SAFE,
                    ..Default::default()
                },
            })
            .await;

        assert!(matches!(
            resume,
            Resume::DynamicCheckResult {
                request_id,
                outcome: CheckOutcome::Approved,
            } if request_id == REQUEST_ID
        ));
    }

    #[tokio::test]
    async fn falls_back_to_the_engine_when_local_checkers_abstain() {
        let handler = Handler::new(
            vec![Box::new(StubChecker(CheckOutcome::Unknown))],
            engine(r#"{"verdict":"secure"}"#).await,
            Duration::from_secs(1),
        );

        let resume = handler
            .perform_effect(Effect::DynamicCheck {
                request_id: REQUEST_ID,
                transaction: SafeTransaction {
                    safe: SAFE,
                    ..Default::default()
                },
            })
            .await;

        assert!(matches!(
            resume,
            Resume::DynamicCheckResult {
                request_id,
                outcome: CheckOutcome::Approved,
            } if request_id == REQUEST_ID
        ));
    }

    // `cow_denial_overrides_an_address_poisoning_approval` deferred to Phase
    // 8a, once `crate::cow::CowChecker` actually exists to exercise.
}
