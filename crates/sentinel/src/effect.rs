//! The sentinel's single effect: requesting a proposed transaction's verdict
//! from the sentinel engine. Emitted by
//! `SentinelTransition::handle_oracle_transaction_proposed` and consumed by
//! `SentinelTransition::apply_transition`'s `Message::Resume` arm.

use crate::{
    bindings::consensus::SafeTransaction,
    engine::{CheckOutcome, EngineClient},
};
use alloy::primitives::B256;
use safenet_core::effects::EffectHandler;
use std::time::Duration;

/// An impure operation the sentinel's state transition asks the [`Handler`]
/// to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Defer the approve/deny decision for `request_id` (a proposed
    /// `transaction` on `safe`) to the configured sentinel engine.
    EngineCheck {
        request_id: B256,
        transaction: SafeTransaction,
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
/// engine for a verdict.
pub struct Handler {
    engine: EngineClient,
    engine_timeout: Duration,
}

impl Handler {
    pub fn new(engine: EngineClient, engine_timeout: Duration) -> Self {
        Self {
            engine,
            engine_timeout,
        }
    }
}

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&self, effect: Effect) -> Resume {
        match effect {
            Effect::EngineCheck {
                request_id,
                transaction,
            } => {
                let outcome = self
                    .engine
                    .security_check(&transaction)
                    .request_id(request_id)
                    .timeout(self.engine_timeout)
                    .execute()
                    .await;
                Resume::EngineCheckResult {
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
    async fn resumes_with_the_engine_outcome() {
        let handler = Handler::new(
            engine(r#"{"verdict":"secure"}"#).await,
            Duration::from_secs(1),
        );

        let resume = handler
            .perform_effect(Effect::EngineCheck {
                request_id: REQUEST_ID,
                transaction: SafeTransaction {
                    safe: SAFE,
                    ..Default::default()
                },
            })
            .await;

        assert!(matches!(
            resume,
            Resume::EngineCheckResult {
                request_id,
                outcome: CheckOutcome::Approved,
            } if request_id == REQUEST_ID
        ));
    }
}
