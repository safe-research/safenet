//! R-4.3/R-4.5 worked example: Safenet's own SAFE-token staking flow —
//! claiming staking rewards and/or staking (optionally re-staking freshly
//! claimed rewards) toward a validator.
//!
//! Each [`claim`](crate::contracts::bindings::staking::claimCall) against the
//! canonical [`REWARDS_DISTRIBUTOR`] is checked on its own: the contract pays
//! `account` — not necessarily `msg.sender` — so it's only accepted when
//! `account` is the Safe itself; otherwise the claimed rewards would be paid
//! to an unrelated address, the same value-misdirection concern
//! [`RuleId::R4_3ValueTarget`] already covers for a plain ERC-20
//! `transfer`/`transferFrom`.
//!
//! What's left after any `claim` calls are set aside is checked as a whole:
//!
//! - A single [`stake`](crate::contracts::bindings::staking::stakeCall)
//!   against [`STAKING`] is secure on its own — it spends an allowance
//!   granted by some earlier, separately-vetted transaction.
//!   `validator` isn't checked here — an unknown or deregistering validator
//!   only affects the staker's own future rewards, not fund safety (the
//!   staked amount stays attributed to the Safe itself in the contract's own
//!   accounting, and can always be withdrawn later).
//! - Exactly one `approve` paired with exactly one `stake`, with the
//!   `approve` running *first* (order matters: an `approve` that runs after
//!   the `stake` it's "paired" with never actually funds it — see
//!   [`check_pair`]), is secure as long as the approved amount doesn't
//!   exceed the staked amount. An approval that *does* exceed it is denied
//!   under [`RuleId::R4_5ExcessiveApproval`] — the approval was consumed by
//!   the right call, just for more than that call needed. An approval for
//!   *less* is fine; the extra must come from an existing allowance.
//!
//! A dangling `approve` — standalone, or paired with a `stake` in the wrong
//! order so nothing in this transaction actually spends it — is left to
//! [`Verdict::Abstain`] here rather than denied: unused-authorization checks
//! generic to any ERC-20 `approve` (not specific to Safenet staking) belong
//! in a separate, general-purpose approval check, not duplicated in this
//! protocol-specific one.
//!
//! Deliberately narrow beyond that: e.g. two `approve` calls in one batch
//! are *not* summed, since a Safe `approve` sets an allowance rather than
//! incrementing it (the second overwrites the first, so the batch's actual
//! net effect depends on execution order in a way that isn't safe to
//! recover from calldata alone). Anything other than the shapes above —
//! more than one `approve`, more than one `stake`, or any unrelated call —
//! is left to [`Verdict::Abstain`] rather than guessed at either way,
//! consistent with this codebase's other protocol-specific worked examples.

use super::Checker;
use crate::{
    contracts::{
        bindings::{
            erc20::approveCall,
            staking::{claimCall, stakeCall},
        },
        multi_send::sub_transactions,
    },
    engine::{Operation, RuleId, SafeTransaction, Verdict},
};
use alloy::{
    primitives::{Address, U256, address},
    sol_types::SolCall as _,
};

/// Safenet's canonical SAFE-token staking contract on Ethereum mainnet (see
/// `docs/configuration.md`'s `STAKER_ADDRESS` section for the Etherscan
/// link).
const STAKING: Address = address!("115E78f160e1E3eF163B05C84562Fa16fA338509");

/// The SAFE token's own canonical address on Ethereum mainnet.
const SAFE_TOKEN: Address = address!("5aFE3855358E112B5647B952709E6165e1c1eEEe");

/// Safe Foundation's canonical cumulative-claim staking-rewards distributor
/// on Ethereum mainnet (`token() == SAFE_TOKEN`, confirmed independently of
/// this codebase).
const REWARDS_DISTRIBUTOR: Address = address!("e5139fc0fb8eae81e30d8a85c22e88c6757120f2");

/// The only chain these canonical addresses are recognized on.
const SUPPORTED_CHAIN_ID: u64 = 1;

/// The built-in Safenet staking check (see module docs).
pub struct StakingChecker;

#[async_trait::async_trait]
impl Checker for StakingChecker {
    fn name(&self) -> &'static str {
        "staking"
    }

    async fn check(&self, transaction: &SafeTransaction) -> Verdict {
        if transaction.chain_id != U256::from(SUPPORTED_CHAIN_ID) {
            return Verdict::Abstain;
        }

        let calls = sub_transactions(transaction);

        let mut remaining = Vec::with_capacity(calls.len());
        let mut claimed = false;
        for call in &calls {
            match claim_account(call) {
                Some(account) if account != transaction.safe => {
                    return Verdict::Insecure {
                        rule: RuleId::R4_3ValueTarget,
                    };
                }
                Some(_) => claimed = true,
                None => remaining.push(call),
            }
        }

        match remaining.as_slice() {
            [] if claimed => Verdict::Secure,
            [] => Verdict::Abstain,
            [call] => check_lone_call(call),
            [first, second] => check_pair(first, second),
            _ => Verdict::Abstain,
        }
    }
}

/// A single non-`claim` call left over after set-aside `claim`s: secure if
/// it's a `stake` (spending some earlier, separately-vetted allowance).
/// A dangling, unused `approve` on [`STAKING`] is left to
/// [`Verdict::Abstain`] — see the module docs for why this check doesn't
/// deny it.
fn check_lone_call(call: &SafeTransaction) -> Verdict {
    if stake_amount(call).is_some() {
        return Verdict::Secure;
    }
    Verdict::Abstain
}

/// Checks an exactly-two-call remainder, in the order the batch itself runs
/// them — order matters here, unlike a same-transaction amount comparison:
/// `[approve, stake]` funds the `stake` with an allowance set moments
/// earlier in the same transaction, but `[stake, approve]` runs the `stake`
/// against whatever allowance already existed *before* this transaction,
/// leaving the `approve` a dangling, un-consumed authorization — left to
/// [`Verdict::Abstain`] for the same reason as [`check_lone_call`]'s
/// standalone `approve` case, regardless of the amount approved.
fn check_pair(first: &SafeTransaction, second: &SafeTransaction) -> Verdict {
    if let (Some(approved), Some(staked)) = (staking_approval_amount(first), stake_amount(second)) {
        return if approved > staked {
            Verdict::Insecure {
                rule: RuleId::R4_5ExcessiveApproval,
            }
        } else {
            Verdict::Secure
        };
    }
    Verdict::Abstain
}

/// The claimed-for account, if `tx` is a `claim` call against
/// [`REWARDS_DISTRIBUTOR`]. Only a plain, valueless `CALL` is recognized — a
/// `DELEGATECALL` executes the target's code in the Safe's own storage
/// context rather than a real call to it, and `claim` is never payable.
fn claim_account(tx: &SafeTransaction) -> Option<Address> {
    if tx.operation != Operation::Call || !tx.value.is_zero() || tx.to != REWARDS_DISTRIBUTOR {
        return None;
    }
    Some(claimCall::abi_decode(&tx.data).ok()?.account)
}

/// The approved amount, if `tx` is an ERC-20 `approve` on [`SAFE_TOKEN`]
/// naming [`STAKING`] as spender. See [`claim_account`] for why
/// `DELEGATECALL` and nonzero `tx.value` are excluded even to a legitimate
/// address.
fn staking_approval_amount(tx: &SafeTransaction) -> Option<U256> {
    if tx.operation != Operation::Call || !tx.value.is_zero() || tx.to != SAFE_TOKEN {
        return None;
    }
    let call = approveCall::abi_decode(&tx.data).ok()?;
    (call.spender == STAKING).then_some(call.amount)
}

/// The staked amount, if `tx` is a `stake` call against [`STAKING`]. See
/// [`claim_account`] for why `DELEGATECALL` and nonzero `tx.value` are
/// excluded even to a legitimate address.
fn stake_amount(tx: &SafeTransaction) -> Option<U256> {
    if tx.operation != Operation::Call || !tx.value.is_zero() || tx.to != STAKING {
        return None;
    }
    Some(stakeCall::abi_decode(&tx.data).ok()?.amount)
}
