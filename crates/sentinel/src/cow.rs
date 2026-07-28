//! R-4.4 worked example (epic Phase 7a): CoW Protocol's own expected
//! transaction shape. A genuine CoW Swap interaction is never a standalone
//! ERC-20 `approve` — the Safe's own frontend always batches the `approve`
//! (to CoW's canonical `GPv2VaultRelayer`) together with the call that
//! actually commits the Safe to an order: a swap's `setPreSignature`
//! presignature call to `GPv2Settlement`, or a TWAP order's creation call
//! (`ComposableCoW.create`, `handler` set to the canonical TWAP contract).
//!
//! This registry (`GPv2VaultRelayer`/`GPv2Settlement`/`ComposableCoW`/TWAP
//! handler addresses) is deliberately sentinel-local, not part of
//! `safe-tx`: `safe-tx`'s own allow-lists are Safe-native protocol constants
//! shared by both `validator` and `sentinel`, whereas these are one
//! third-party dapp's addresses that only this one check needs today.
//!
//! **Denies, rather than exempts.** A standalone `approve` to
//! `GPv2VaultRelayer` — not co-batched with a recognized trigger call — is
//! denied under [`RuleId::R4_4AuthorizationTarget`], including when
//! [`crate::address_poisoning::AddressPoisoningChecker`] would otherwise have
//! approved the same target off a prior genuine interaction: this
//! protocol-specific rule runs ahead of, and overrides, that general
//! history-based bypass (`CowChecker` is ordered first in
//! `crate::effect::Handler`'s checker chain). A properly batched `approve`
//! is `Unknown` for now — checking that its amount actually matches the
//! order it funds is deferred to a follow-up PR (see
//! [`CowChecker::check_presignature_batch`]/[`CowChecker::check_twap_batch`]).

use crate::checker::{CheckOutcome, Checker};
use alloy::{
    primitives::{Address, U256, address},
    sol,
    sol_types::SolCall,
};
use safe_tx::{
    multi_send::decode_multi_send_call,
    rule::RuleId,
    types::{Operation, SafeTransaction, erc20::approveCall},
};

sol! {
    function setPreSignature(bytes orderUid, bool signed);

    struct ConditionalOrderParams {
        address handler;
        bytes32 salt;
        bytes staticInput;
    }
    function create(ConditionalOrderParams params, bool dispatch);
}

/// CoW Protocol's canonical contracts. Deployed at the same address on every
/// network CoW Swap supports (deterministic `CREATE2` deployment), including
/// all three networks the Charter scopes to (Article I).
const GP_V2_VAULT_RELAYER: Address = address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110");
const GP_V2_SETTLEMENT: Address = address!("9008D19f58AAbD9eD0D60971565AA8510560ab41");
const COMPOSABLE_COW: Address = address!("fdaFc9d1902f4e0b84f65F49f244b32b31013b74");
const TWAP_HANDLER: Address = address!("6cF1e9cA41f7611dEf408122793c358a3d11E5a5");

/// Chain IDs the above are recognized on: Ethereum mainnet, Arbitrum One,
/// Gnosis Chain.
const SUPPORTED_CHAIN_IDS: &[u64] = &[1, 42161, 100];

/// The built-in CoW Swap check (see module docs). Holds no state — the
/// registry above is entirely static — but is a struct (rather than a bare
/// function) so it can implement [`Checker`] and slot into
/// `crate::effect::Handler`'s checker chain like every other check.
pub struct CowChecker;

impl CowChecker {
    pub fn new() -> Self {
        Self {}
    }

    async fn check_presignature_batch(&self, _calls: &[SafeTransaction]) -> CheckOutcome {
        // Currently only placeholder, proper check has to be implemented in follow up PR:
        // - check that batch size is 2 -> if not return unknown
        // - check if one of them is an approval -> if not return unknown
        // - check if other is pre signature call -> if not return unknown
        // - fetch order from CoW API and parse token, amount and receiver
        // - if receiver is different -> DENY
        // - if token is different -> DENY
        // - if token amount is different -> DENY
        // - otherwise APPROVE
        CheckOutcome::Unknown
    }

    async fn check_twap_batch(&self, _calls: &[SafeTransaction]) -> CheckOutcome {
        // Currently only placeholder, proper check has to be implemented in follow up PR:
        // - check that batch size is 2 -> if not return unknown
        // - check if one of them is an approval -> if not return unknown
        // - check if other is twap call -> if not return unknown
        // - decode TWAP order creation
        // - if receiver is different -> DENY
        // - if token is different -> DENY
        // - calculate total amount. if different to approve amount -> DENY
        // - otherwise APPROVE
        CheckOutcome::Unknown
    }

    /// An `approve` to `GPv2VaultRelayer` with no co-batched presignature or
    /// TWAP order-creation call is not the pattern a genuine CoW Swap
    /// interaction takes.
    async fn check_dangling_approval(&self, calls: &[SafeTransaction]) -> CheckOutcome {
        if !calls.iter().any(approves_vault_relayer) {
            return CheckOutcome::Unknown;
        }
        if calls
            .iter()
            .any(|c| is_presignature(c) || is_twap_create(c))
        {
            CheckOutcome::Unknown
        } else {
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        }
    }
}

#[async_trait::async_trait]
impl Checker for CowChecker {
    /// Runs, in order, [`CowChecker::check_dangling_approval`],
    /// [`CowChecker::check_presignature_batch`] and
    /// [`CowChecker::check_twap_batch`] against `transaction`'s sub-calls,
    /// returning the first non-[`CheckOutcome::Unknown`] result.
    async fn check(&self, _safe: Address, transaction: &SafeTransaction) -> CheckOutcome {
        if !SUPPORTED_CHAIN_IDS
            .iter()
            .any(|&id| transaction.chainId == U256::from(id))
        {
            return CheckOutcome::Unknown;
        }
        let calls = sub_transactions(transaction);

        let dangling_check = self.check_dangling_approval(&calls).await;
        if dangling_check != CheckOutcome::Unknown {
            return dangling_check;
        }

        let presig_check = self.check_presignature_batch(&calls).await;
        if presig_check != CheckOutcome::Unknown {
            return presig_check;
        }

        let twap_check = self.check_twap_batch(&calls).await;
        if twap_check != CheckOutcome::Unknown {
            return twap_check;
        }

        CheckOutcome::Unknown
    }
}

/// `tx` itself, or, if it's a MultiSend batch, each of its sub-calls.
fn sub_transactions(tx: &SafeTransaction) -> Vec<SafeTransaction> {
    decode_multi_send_call(tx)
        .map(|(sub_txs, _)| sub_txs)
        .unwrap_or_else(|| vec![tx.clone()])
}

/// Only a plain, valueless `CALL` is recognized — a `DELEGATECALL` executes
/// `tx.to`'s code in the Safe's own storage context rather than a real
/// ERC-20 `approve`, so calldata that merely matches the selector must not
/// be treated as one; and a genuine `approve` never carries native value, so
/// a nonzero `tx.value` is itself a sign this isn't the expected call.
fn approves_vault_relayer(tx: &SafeTransaction) -> bool {
    tx.operation == Operation::CALL
        && tx.value.is_zero()
        && approveCall::abi_decode(&tx.data).is_ok_and(|call| call.spender == GP_V2_VAULT_RELAYER)
}

/// Only a plain, valueless `CALL` to `GPv2Settlement` is recognized — see
/// [`approves_vault_relayer`] for why `DELEGATECALL` and nonzero `tx.value`
/// are excluded even to a legitimate address.
fn is_presignature(tx: &SafeTransaction) -> bool {
    tx.operation == Operation::CALL
        && tx.value.is_zero()
        && tx.to == GP_V2_SETTLEMENT
        && setPreSignatureCall::abi_decode(&tx.data).is_ok()
}

/// Only a plain, valueless `CALL` to `ComposableCoW` is recognized — see
/// [`approves_vault_relayer`] for why `DELEGATECALL` and nonzero `tx.value`
/// are excluded even to a legitimate address.
fn is_twap_create(tx: &SafeTransaction) -> bool {
    tx.operation == Operation::CALL
        && tx.value.is_zero()
        && tx.to == COMPOSABLE_COW
        && createCall::abi_decode(&tx.data).is_ok_and(|call| call.params.handler == TWAP_HANDLER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{B256, Bytes};
    use safe_tx::types::multi_send_bindings;

    const SAFE: Address = Address::new([1u8; 20]);
    const TOKEN: Address = Address::new([2u8; 20]);
    const MULTI_SEND: Address = address!("218543288004CD07832472D464648173c77D7eB7");

    fn tx(to: Address, data: Vec<u8>, operation: Operation) -> SafeTransaction {
        SafeTransaction {
            chainId: U256::from(1u64),
            safe: SAFE,
            to,
            data: data.into(),
            operation,
            ..Default::default()
        }
    }

    fn approve_data(spender: Address) -> Vec<u8> {
        approveCall {
            spender,
            amount: U256::from(1u64),
        }
        .abi_encode()
    }

    fn pack(operation: Operation, to: Address, data: &[u8]) -> Vec<u8> {
        pack_with_value(operation, to, U256::ZERO, data)
    }

    fn pack_with_value(operation: Operation, to: Address, value: U256, data: &[u8]) -> Vec<u8> {
        let mut out = vec![operation as u8];
        out.extend_from_slice(to.as_slice());
        out.extend_from_slice(&value.to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(data.len()).to_be_bytes::<32>());
        out.extend_from_slice(data);
        out
    }

    fn multisend(sub_txs: &[Vec<u8>]) -> Bytes {
        let transactions: Vec<u8> = sub_txs.iter().flatten().cloned().collect();
        Bytes::from(
            multi_send_bindings::multiSendCall {
                transactions: Bytes::from(transactions),
            }
            .abi_encode(),
        )
    }

    async fn check(transaction: &SafeTransaction) -> CheckOutcome {
        CowChecker::new().check(SAFE, transaction).await
    }

    #[tokio::test]
    async fn no_opinion_when_no_relayer_approval_is_present() {
        let data = approve_data(Address::new([9u8; 20]));
        assert_eq!(
            check(&tx(TOKEN, data, Operation::CALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn denies_a_standalone_approval_to_the_relayer() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        assert_eq!(
            check(&tx(TOKEN, data, Operation::CALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn no_opinion_when_approval_is_batched_with_a_presignature_call() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let presig = setPreSignatureCall {
            orderUid: Bytes::from(vec![0xab; 56]),
            signed: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, GP_V2_SETTLEMENT, &presig),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn no_opinion_when_approval_is_batched_with_a_twap_create_call() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let create = createCall {
            params: ConditionalOrderParams {
                handler: TWAP_HANDLER,
                salt: B256::ZERO,
                staticInput: Bytes::new(),
            },
            dispatch: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn denies_a_batched_approval_without_a_recognized_trigger() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let data = multisend(&[pack(Operation::CALL, TOKEN, &approve)]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn denies_when_batch_targets_an_unrecognized_create_handler() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let create = createCall {
            params: ConditionalOrderParams {
                handler: Address::new([7u8; 20]),
                salt: B256::ZERO,
                staticInput: Bytes::new(),
            },
            dispatch: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    // A `DELEGATECALL` executes `tx.to`'s code in the Safe's own storage
    // context rather than a real ERC-20 `approve`, so calldata that merely
    // matches the selector must not be treated as one.
    #[tokio::test]
    async fn no_opinion_when_the_approval_itself_is_a_delegatecall() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        assert_eq!(
            check(&tx(TOKEN, data, Operation::DELEGATECALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn denies_when_the_presignature_trigger_is_a_delegatecall() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let presig = setPreSignatureCall {
            orderUid: Bytes::from(vec![0xab; 56]),
            signed: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::DELEGATECALL, GP_V2_SETTLEMENT, &presig),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn denies_when_the_twap_create_trigger_is_a_delegatecall() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let create = createCall {
            params: ConditionalOrderParams {
                handler: TWAP_HANDLER,
                salt: B256::ZERO,
                staticInput: Bytes::new(),
            },
            dispatch: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::DELEGATECALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    // A genuine `approve` never carries native value.
    #[tokio::test]
    async fn no_opinion_when_the_approval_carries_native_value() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        let transaction = SafeTransaction {
            value: U256::from(1u64),
            ..tx(TOKEN, data, Operation::CALL)
        };
        assert_eq!(check(&transaction).await, CheckOutcome::Unknown);
    }

    #[tokio::test]
    async fn denies_when_the_presignature_trigger_carries_native_value() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let presig = setPreSignatureCall {
            orderUid: Bytes::from(vec![0xab; 56]),
            signed: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack_with_value(Operation::CALL, GP_V2_SETTLEMENT, U256::from(1u64), &presig),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn denies_when_the_twap_create_trigger_carries_native_value() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let create = createCall {
            params: ConditionalOrderParams {
                handler: TWAP_HANDLER,
                salt: B256::ZERO,
                staticInput: Bytes::new(),
            },
            dispatch: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack_with_value(Operation::CALL, COMPOSABLE_COW, U256::from(1u64), &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn no_opinion_on_an_unsupported_chain() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        let transaction = SafeTransaction {
            chainId: U256::from(137u64),
            ..tx(TOKEN, data, Operation::CALL)
        };
        assert_eq!(check(&transaction).await, CheckOutcome::Unknown);
    }
}
