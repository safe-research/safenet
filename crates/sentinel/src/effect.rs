//! The sentinel's single effect: deferring a proposed transaction's
//! remaining approve/deny decision to
//! [`crate::dynamic_checker::RemoteChecker`] once the local, synchronous
//! checks in [`crate::static_checker::StaticChecker`] have passed. Emitted by
//! `SentinelTransition::handle_oracle_transaction_proposed` and consumed by
//! `SentinelTransition::apply_transition`'s `Message::Resume` arm.

use crate::dynamic_checker::{RemoteCheckOutcome, RemoteChecker};
use alloy::primitives::{Address, B256};
use safe_tx::types::SafeTransaction;
use safenet_core::state::EffectHandler;

/// An impure operation the sentinel's state transition asks the [`Handler`]
/// to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Defer the approve/deny decision for `request_id` (a proposed
    /// `transaction` on `safe`) to the configured dynamic check. `deadline`
    /// is carried through unchanged to the resulting [`Resume`] — the
    /// [`Handler`] never reads it, it's just along for the ride so
    /// `SentinelTransition` doesn't need to persist any intermediate state
    /// of its own while this effect is outstanding.
    DynamicCheck {
        request_id: B256,
        safe: Address,
        transaction: SafeTransaction,
        deadline: u64,
    },
}

/// The result of performing an [`Effect`], resumed into the state machine.
#[derive(Debug, Clone)]
pub enum Resume {
    /// Resume with [`Effect::DynamicCheck`]'s outcome for `request_id`,
    /// carrying its `deadline` back unchanged.
    DynamicCheckResult {
        request_id: B256,
        deadline: u64,
        outcome: RemoteCheckOutcome,
    },
}

/// Performs the sentinel's [`Effect`]s against the configured dynamic check.
pub struct Handler {
    checker: RemoteChecker,
}

impl Handler {
    pub fn new(checker: RemoteChecker) -> Self {
        Self { checker }
    }
}

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&mut self, effect: Effect) -> Resume {
        match effect {
            Effect::DynamicCheck {
                request_id,
                safe,
                transaction,
                deadline,
            } => Resume::DynamicCheckResult {
                request_id,
                deadline,
                outcome: self.checker.check(safe, &transaction).await,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAFE: Address = Address::new([1u8; 20]);
    const REQUEST_ID: B256 = B256::repeat_byte(0x11);

    #[tokio::test]
    async fn resumes_with_the_checker_s_outcome() {
        // An unconfigured `RemoteChecker` always approves; this only
        // exercises the `Effect` -> `Resume` wiring itself.
        let mut handler = Handler::new(RemoteChecker::new(None));

        let resume = handler
            .perform_effect(Effect::DynamicCheck {
                request_id: REQUEST_ID,
                safe: SAFE,
                transaction: SafeTransaction::default(),
                deadline: 42,
            })
            .await;

        assert!(matches!(
            resume,
            Resume::DynamicCheckResult {
                request_id,
                deadline: 42,
                outcome: RemoteCheckOutcome::Approved,
            } if request_id == REQUEST_ID
        ));
    }
}
