//! R-4.3/R-4.4 address-poisoning check (first step): whether an
//! ERC-20 `transfer`/`transferFrom`/`approve` target has a prior genuine
//! interaction with the Safe. Unlike [`crate::static_checker::StaticChecker`],
//! this inherently needs onchain event history, so it runs as part of
//! [`crate::effect::Effect::DynamicCheck`] via a `Provider` held here,
//! reusing the same one `main.rs` already constructs for event-watching.
//!
//! Scoped narrowly for this first step: only a single (non-MultiSend) ERC-20
//! call is decoded, and only the exact `(safe, candidate)` pair is queried —
//! not the Safe's full recipient history — over an operator-configured,
//! bounded block range (a single `eth_getLogs` call, no pagination). Native
//! value transfers and ERC-721/1155 transfers are out of scope for now.
//!
//! **This check does not yet deny anything.** A found prior event is
//! unforgeable evidence of an established relationship, so it approves
//! immediately — no further check (including the operator's configured
//! `RemoteChecker`) needs to run. Not finding one is inconclusive rather
//! than suspicious: a single exact-match lookup isn't strong enough evidence
//! on its own to deny a first-time-looking recipient (and the Charter's own
//! §2.4 Notes caution against treating novelty alone as grounds for denial),
//! so that case defers to whatever check runs next. See the `TODO` on
//! [`AddressPoisoningChecker`]'s [`Checker`] impl.

use crate::{
    bindings::consensus::SafeTransaction,
    checker::{CheckOutcome, Checker},
};
use alloy::{
    primitives::Address,
    providers::{DynProvider, Provider},
    rpc::types::Filter,
    sol,
    sol_types::{SolCall, SolEvent},
};
use safe_tx::{
    bindings::erc20::{approveCall, transferCall, transferFromCall},
    rule::RuleId,
};

sol! {
    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);
}

/// Which of the two poisoning-relevant ERC-20 call shapes `tx.data` decoded
/// to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    /// `transfer`/`transferFrom` — maps to [`RuleId::R4_3ValueTarget`].
    Transfer,
    /// `approve` — maps to [`RuleId::R4_4AuthorizationTarget`].
    Approval,
}

impl TargetKind {
    fn rule(self) -> RuleId {
        match self {
            Self::Transfer => RuleId::R4_3ValueTarget,
            Self::Approval => RuleId::R4_4AuthorizationTarget,
        }
    }

    fn event_signature(self) -> alloy::primitives::B256 {
        match self {
            Self::Transfer => Transfer::SIGNATURE_HASH,
            Self::Approval => Approval::SIGNATURE_HASH,
        }
    }
}

/// Decodes `tx.data` as an ERC-20 `transfer`/`transferFrom`/`approve` call,
/// returning the recipient/spender address and which kind it is. `None` for
/// anything else — native value, unrelated calldata, or a MultiSend batch
/// (recursing through batched sub-calls is deferred; see module docs).
fn decode_target(tx: &SafeTransaction) -> Option<(Address, TargetKind)> {
    if let Ok(call) = transferCall::abi_decode(&tx.data) {
        return Some((call.to, TargetKind::Transfer));
    }
    if let Ok(call) = transferFromCall::abi_decode(&tx.data) {
        return Some((call.to, TargetKind::Transfer));
    }
    if let Ok(call) = approveCall::abi_decode(&tx.data) {
        return Some((call.spender, TargetKind::Approval));
    }
    None
}

/// Runs the R-4.3/R-4.4 address-poisoning check (see module docs for its
/// current scope and limitations).
pub struct AddressPoisoningChecker {
    provider: DynProvider,
    lookback_blocks: u64,
}

impl AddressPoisoningChecker {
    pub fn new(provider: DynProvider, lookback_blocks: u64) -> Self {
        Self {
            provider,
            lookback_blocks,
        }
    }

    /// Whether `token` already emitted a genuine `Transfer`/`Approval` event
    /// from `safe` to `candidate`, within `lookback_blocks` of the current
    /// block. A single bounded `eth_getLogs` call, not a history scan.
    async fn has_prior_interaction(
        &self,
        token: Address,
        safe: Address,
        candidate: Address,
        kind: TargetKind,
    ) -> Result<bool, alloy::transports::TransportError> {
        // `from_block` needs a concrete number (no JSON-RPC tag means "N
        // blocks before the tip"), but `to_block` doesn't — asking for
        // `Latest` there lets the node resolve the tip itself rather than
        // pinning it to whatever `get_block_number` happened to return.
        let current_block = self.provider.get_block_number().await?;
        let from_block = current_block.saturating_sub(self.lookback_blocks);
        let filter = Filter::new()
            .address(token)
            .event_signature(kind.event_signature())
            .topic1(safe)
            .topic2(candidate)
            .from_block(from_block)
            .to_block(alloy::eips::BlockNumberOrTag::Latest);
        Ok(!self.provider.get_logs(&filter).await?.is_empty())
    }
}

#[async_trait::async_trait]
impl Checker for AddressPoisoningChecker {
    /// Decodes `transaction.data`'s ERC-20 target (if any) and looks up
    /// whether the Safe already has a genuine prior interaction with it. A
    /// found interaction approves outright; anything else (no ERC-20 target
    /// to check, no prior interaction found, or the lookup itself failing)
    /// is `Unknown`, deferring to whatever checker runs next.
    ///
    /// TODO(follow-up): an `Unknown` verdict never becomes a
    /// denial yet — richer recipient-quality signals (the candidate's own
    /// fund/transaction history, whether it's an EOA or a contract, and if
    /// so its deployment age) are needed before a first-time-looking
    /// recipient can be safely denied.
    async fn check(&self, transaction: &SafeTransaction) -> CheckOutcome {
        let Some((candidate, kind)) = decode_target(transaction) else {
            return CheckOutcome::Unknown;
        };
        match self
            .has_prior_interaction(transaction.to, transaction.safe, candidate, kind)
            .await
        {
            Ok(found) => {
                tracing::debug!(token = %transaction.to, %candidate, found, rule = kind.rule().code(), "address-poisoning history lookup");
                if found {
                    CheckOutcome::Approved
                } else {
                    CheckOutcome::Unknown
                }
            }
            Err(err) => {
                tracing::warn!(%err, token = %transaction.to, %candidate, rule = kind.rule().code(), "address-poisoning history lookup failed");
                CheckOutcome::Unknown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{primitives::U256, providers::ProviderBuilder, transports::mock::Asserter};

    const SAFE: Address = Address::new([1u8; 20]);
    const TOKEN: Address = Address::new([2u8; 20]);
    const CANDIDATE: Address = Address::new([3u8; 20]);

    fn checker(asserter: &Asserter) -> AddressPoisoningChecker {
        let provider = ProviderBuilder::default()
            .connect_mocked_client(asserter.clone())
            .erased();
        AddressPoisoningChecker::new(provider, 1_000)
    }

    fn tx(to: Address, data: Vec<u8>) -> SafeTransaction {
        SafeTransaction {
            safe: SAFE,
            to,
            data: data.into(),
            ..Default::default()
        }
    }

    #[test]
    fn decodes_transfer_and_transfer_from_as_target_kind_transfer() {
        let data = transferCall {
            to: CANDIDATE,
            amount: U256::from(1u64),
        }
        .abi_encode();
        assert_eq!(
            decode_target(&tx(TOKEN, data)),
            Some((CANDIDATE, TargetKind::Transfer))
        );

        let data = transferFromCall {
            from: SAFE,
            to: CANDIDATE,
            amount: U256::from(1u64),
        }
        .abi_encode();
        assert_eq!(
            decode_target(&tx(TOKEN, data)),
            Some((CANDIDATE, TargetKind::Transfer))
        );
    }

    #[test]
    fn decodes_approve_as_target_kind_approval() {
        let data = approveCall {
            spender: CANDIDATE,
            amount: U256::from(1u64),
        }
        .abi_encode();
        assert_eq!(
            decode_target(&tx(TOKEN, data)),
            Some((CANDIDATE, TargetKind::Approval))
        );
    }

    #[test]
    fn decodes_unrelated_calldata_as_none() {
        assert_eq!(
            decode_target(&tx(TOKEN, vec![0xde, 0xad, 0xbe, 0xef])),
            None
        );
        assert_eq!(decode_target(&tx(TOKEN, vec![])), None);
    }

    #[tokio::test]
    async fn no_opinion_when_no_prior_event_is_found() {
        let asserter = Asserter::new();
        asserter.push_success(&alloy::primitives::U64::from(1_000u64));
        asserter.push_success(&Vec::<alloy::rpc::types::Log>::new());

        let data = approveCall {
            spender: CANDIDATE,
            amount: U256::from(1u64),
        }
        .abi_encode();
        let verdict = checker(&asserter).check(&tx(TOKEN, data)).await;
        assert_eq!(verdict, CheckOutcome::Unknown);
    }

    #[tokio::test]
    async fn approved_when_a_prior_event_is_found() {
        let asserter = Asserter::new();
        asserter.push_success(&alloy::primitives::U64::from(1_000u64));
        let logs: Vec<alloy::rpc::types::Log> = vec![Default::default()];
        asserter.push_success(&logs);

        let data = approveCall {
            spender: CANDIDATE,
            amount: U256::from(1u64),
        }
        .abi_encode();
        let verdict = checker(&asserter).check(&tx(TOKEN, data)).await;
        assert_eq!(verdict, CheckOutcome::Approved);
    }

    #[tokio::test]
    async fn no_opinion_when_calldata_is_out_of_scope() {
        // No RPC calls should even be issued when there's nothing to decode.
        let asserter = Asserter::new();
        let verdict = checker(&asserter).check(&tx(TOKEN, vec![])).await;
        assert_eq!(verdict, CheckOutcome::Unknown);
    }
}
