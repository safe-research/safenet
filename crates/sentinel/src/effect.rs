//! The sentinel's single effect: deferring a proposed transaction's
//! remaining approve/deny decision to [`Handler`]'s checker chain once the
//! local, synchronous checks in [`crate::static_checker::StaticChecker`]
//! have passed. Emitted by
//! `SentinelTransition::handle_oracle_transaction_proposed` and consumed by
//! `SentinelTransition::apply_transition`'s `Message::Resume` arm.

use crate::{
    bindings::consensus::SafeTransaction,
    checker::{CheckOutcome, Checker},
};
use alloy::primitives::B256;
use safenet_core::effects::EffectHandler;

/// An impure operation the sentinel's state transition asks the [`Handler`]
/// to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Defer the approve/deny decision for `request_id` (a proposed
    /// `transaction` on `safe`) to the configured dynamic check.
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

/// Performs the sentinel's [`Effect`]s by running its [`Checker`] chain in
/// order, stopping at the first non-[`CheckOutcome::Unknown`] result. If
/// every checker resolves to `Unknown`, so does the whole effect — see
/// `crate::checker` for why that's the right default rather than guessing.
///
/// This is an initial cut of the "array of checkers" shape (see
/// `crate::checker`'s module docs); the fixed construction order below is
/// what encodes today's precedence (the built-in CoW check ahead of
/// address-poisoning, ahead of the operator-configured remote check).
pub struct Handler {
    checkers: Vec<Box<dyn Checker>>,
}

impl Handler {
    pub fn new(checkers: Vec<Box<dyn Checker>>) -> Self {
        Self { checkers }
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
    use crate::{address_poisoning::AddressPoisoningChecker, dynamic_checker::RemoteChecker};
    use alloy::{
        primitives::Address,
        providers::{Provider, ProviderBuilder},
        transports::mock::Asserter,
    };

    const SAFE: Address = Address::new([1u8; 20]);
    const REQUEST_ID: B256 = B256::repeat_byte(0x11);

    fn address_poisoning() -> AddressPoisoningChecker {
        let provider = ProviderBuilder::default()
            .connect_mocked_client(Asserter::new())
            .erased();
        AddressPoisoningChecker::new(provider, 1_000)
    }

    fn handler(address_poisoning: AddressPoisoningChecker, remote: RemoteChecker) -> Handler {
        Handler::new(vec![Box::new(address_poisoning), Box::new(remote)])
    }

    #[tokio::test]
    async fn resumes_with_the_checker_s_outcome() {
        // An unconfigured `RemoteChecker` always approves, and a
        // transaction with only its Safe set has no ERC-20 calldata for the
        // earlier checkers to inspect — this only exercises the `Effect` ->
        // `Resume` wiring itself.
        let handler = handler(address_poisoning(), RemoteChecker::new(None));

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
