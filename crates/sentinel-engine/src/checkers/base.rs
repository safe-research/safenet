//! Article IV Part A base guarantees.

use super::{Assessment, Checker};
use crate::{
    contracts::bindings::safe,
    engine::{AspectSet, CheckContext, MetaTransaction, Operation, Proposal, RuleId},
};
use alloy::{
    primitives::{Address, address},
    sol_types::SolCall as _,
};

const SUPPORTED_FALLBACK_HANDLERS: &[Address] = &[
    Address::ZERO,
    address!("85a8ca358D388530ad0fB95D0cb89Dd44Fc242c3"),
    address!("2f55e8b20D0B9FEFA187AA7d00B6Cbe563605bF5"),
    address!("3EfCBb83A4A7AfcB4F68D501E2c2203a38be77f4"),
    address!("fd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99"),
    address!("f48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4"),
    address!("017062a1dE2FE6b99BE3d9d37841FeD19F573804"),
];

const SUPPORTED_GUARDS: &[Address] = &[Address::ZERO];

const SUPPORTED_MODULES: &[Address] = &[
    address!("691f59471Bfd2B7d639DCF74671a2d648ED1E331"),
    address!("4Aa5Bf7D840aC607cb5BD3249e6Af6FC86C04897"),
];

const SUPPORTED_MODULE_GUARDS: &[Address] = &[Address::ZERO];

/// Enforces the Safe base guarantees from Article IV Part A.
///
/// The sole supplier of `To` and `Operation` coverage for ordinary calls,
/// on the strength of **"no rule in scope forbids this destination" — not
/// "this destination is trustworthy."** A call that passes has had its `to`
/// evaluated against every `to`-restriction the Charter currently states (a
/// self-call confined to an allow-listed settings function, a delegatecall
/// confined to a known migration, signing-library, `CreateCall` or
/// MultiSend contract), and `BlocklistChecker` (R-4.6) has had its chance to
/// deny. That is the whole of what this claim asserts; a positive statement
/// about the destination is tracked as F2 (positive destination assurance)
/// in the verdict-composition epic.
///
/// This check never abstains — see [`check_call`].
pub struct BaseChecker;

#[async_trait::async_trait]
impl Checker for BaseChecker {
    fn name(&self) -> &'static str {
        "base"
    }

    async fn check(&self, proposal: &Proposal, _context: &CheckContext) -> Assessment {
        let safe = proposal.transaction.safe;
        match proposal
            .calls
            .iter()
            .try_for_each(|call| check_call(safe, call))
        {
            Ok(()) => Assessment::Secure {
                coverage: proposal.checked(AspectSet::TO | AspectSet::OPERATION),
            },
            Err(rule) => Assessment::Insecure { rule },
        }
    }
}

/// Checks one call against the Article IV Part A base guarantees
/// (settings-change blocking, delegatecall integrity). On denial, returns
/// the violated rule — `call`'s own, whether `call` is the top-level call or
/// one leg of a flattened MultiSend batch, so a batch's failing sub-call is
/// cited under its own rule rather than the container's R-4.2.
///
/// Always answers `Ok` or `Err`, never neither: `Operation` has exactly two
/// variants, and `check_settings_change` and `check_delegatecall_integrity`
/// each return `Some` for their own variant, so the `.unwrap_or(..)`
/// fallback below is unreachable. `BaseChecker::check` relies on this to
/// never abstain.
fn check_call(safe: Address, call: &MetaTransaction) -> Result<(), RuleId> {
    check_settings_change(safe, call)
        .or_else(|| check_delegatecall_integrity(call))
        .unwrap_or(Err(RuleId::R4_1SettingsChange))
}

/// Article IV Part A settings-change guarantee. `None` if `call` isn't a
/// self-call at all — not this rule's concern.
fn check_settings_change(safe: Address, call: &MetaTransaction) -> Option<Result<(), RuleId>> {
    if call.operation != Operation::Call {
        return None;
    }
    Some(if check_calls(safe, call) {
        Ok(())
    } else {
        Err(RuleId::R4_1SettingsChange)
    })
}

/// Article IV Part A delegatecall-integrity guarantee. `None` if `call` isn't
/// a delegatecall at all — not this rule's concern. A delegatecall to a
/// known MultiSend contract is allowed here unconditionally: the engine's
/// entry-point parser already flattened a recognized batch into its own
/// entries of `proposal.calls`, so by the time this runs, a delegatecall
/// still carrying a MultiSend `to` is either the container of a batch that
/// failed to flatten (malformed payload, or a deployment that disallows the
/// sub-call's own operation) and is correctly denied as an unknown
/// delegatecall, or is unreachable because it was replaced by its sub-calls.
fn check_delegatecall_integrity(call: &MetaTransaction) -> Option<Result<(), RuleId>> {
    if call.operation != Operation::DelegateCall {
        return None;
    }
    Some(if check_delegate_calls(call) {
        Ok(())
    } else {
        Err(RuleId::R4_2DelegatecallIntegrity)
    })
}

/// Calls to other contracts are freely allowed; self-calls are restricted to a
/// whitelist of Safe management functions.
fn check_calls(safe: Address, call: &MetaTransaction) -> bool {
    if call.operation != Operation::Call {
        return false;
    }
    if safe != call.to {
        return true;
    }
    // receive: empty calldata with any value (e.g. cancellation transactions).
    if call.data.is_empty() {
        return true;
    }
    check_self_calls(call)
}

/// Checks that a self-call targets one of the allowed Safe management
/// functions (with argument validation where necessary).
fn check_self_calls(call: &MetaTransaction) -> bool {
    // No-arg checks: any calldata starting with the right selector is allowed.
    if call
        .data
        .starts_with(&safe::addOwnerWithThresholdCall::SELECTOR)
    {
        return true;
    }
    if call.data.starts_with(&safe::removeOwnerCall::SELECTOR) {
        return true;
    }
    if call.data.starts_with(&safe::swapOwnerCall::SELECTOR) {
        return true;
    }
    if call.data.starts_with(&safe::changeThresholdCall::SELECTOR) {
        return true;
    }
    if call.data.starts_with(&safe::disableModuleCall::SELECTOR) {
        return true;
    }

    // Arg-validated checks: the first address argument must be in the allow-list.
    if call
        .data
        .starts_with(&safe::setFallbackHandlerCall::SELECTOR)
    {
        return safe::setFallbackHandlerCall::abi_decode(&call.data)
            .ok()
            .is_some_and(|decoded| SUPPORTED_FALLBACK_HANDLERS.contains(&decoded.handler));
    }
    if call.data.starts_with(&safe::setGuardCall::SELECTOR) {
        return safe::setGuardCall::abi_decode(&call.data)
            .ok()
            .is_some_and(|decoded| SUPPORTED_GUARDS.contains(&decoded.guard));
    }
    if call.data.starts_with(&safe::enableModuleCall::SELECTOR) {
        return safe::enableModuleCall::abi_decode(&call.data)
            .ok()
            .is_some_and(|decoded| SUPPORTED_MODULES.contains(&decoded.module));
    }
    if call.data.starts_with(&safe::setModuleGuardCall::SELECTOR) {
        return safe::setModuleGuardCall::abi_decode(&call.data)
            .ok()
            .is_some_and(|decoded| SUPPORTED_MODULE_GUARDS.contains(&decoded.guard));
    }

    false
}

/// Delegate calls are restricted to known Safe migration and signing-library
/// contracts, each with a fixed set of allowed function selectors.
fn check_delegate_calls(call: &MetaTransaction) -> bool {
    if call.operation != Operation::DelegateCall {
        return false;
    }

    const MIGRATION_CONTRACTS: &[Address] = &[
        address!("6439e7ABD8Bb915A5263094784C5CF561c4172AC"),
        address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
    ];
    if MIGRATION_CONTRACTS.contains(&call.to) {
        return call.data.starts_with(&safe::migrateSingletonCall::SELECTOR)
            || call
                .data
                .starts_with(&safe::migrateWithFallbackHandlerCall::SELECTOR)
            || call
                .data
                .starts_with(&safe::migrateL2SingletonCall::SELECTOR)
            || call
                .data
                .starts_with(&safe::migrateL2WithFallbackHandlerCall::SELECTOR);
    }

    const SIGN_MESSAGE_LIBS: &[Address] = &[
        address!("A65387F16B013cf2Af4605Ad8aA5ec25a2cbA3a2"),
        address!("98FFBBF51bb33A056B08ddf711f289936AafF717"),
        address!("d53cd0aB83D845Ac265BE939c57F53AD838012c9"),
        address!("4FfeF8222648872B3dE295Ba1e49110E61f5b5aa"),
    ];
    if SIGN_MESSAGE_LIBS.contains(&call.to) {
        return call.data.starts_with(&safe::signMessageCall::SELECTOR);
    }

    const CREATE_CALL_CONTRACTS: &[Address] = &[
        address!("7cbB62EaA69F79e6873cD1ecB2392971036cFAa4"), // 1.3.0 - canonical
        address!("B19D6FFc2182150F8Eb585b79D4ABcd7C5640A9d"), // 1.3.0 - eip155
        address!("9b35Af71d77eaf8d7e40252370304687390A1A52"), // 1.4.1
        address!("2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4"), // 1.5.0
    ];
    if CREATE_CALL_CONTRACTS.contains(&call.to) {
        return call.data.starts_with(&safe::performCreateCall::SELECTOR)
            || call.data.starts_with(&safe::performCreate2Call::SELECTOR);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Coverage, SafeTransaction};
    use alloy::primitives::{Address, Bytes, U256, address};

    /// Checks a single call's four action fields against the base
    /// guarantees, bypassing `Proposal`/`Checker::check` for tests that only
    /// care about `check_call`'s own rule logic.
    fn check(
        safe: Address,
        to: Address,
        value: U256,
        data: impl Into<Bytes>,
        operation: Operation,
    ) -> Result<(), RuleId> {
        check_call(
            safe,
            &MetaTransaction {
                to,
                value,
                data: data.into(),
                operation,
            },
        )
    }

    fn hex(s: &str) -> Bytes {
        let s = s.strip_prefix("0x").unwrap_or(s);
        Bytes::from(alloy::primitives::hex::decode(s).expect("invalid hex"))
    }

    #[tokio::test]
    async fn denies_self_call_not_on_settings_allow_list() {
        let safe = Address::new([1u8; 20]);
        let transaction = SafeTransaction {
            safe,
            to: safe,
            data: vec![0xde, 0xad, 0xbe, 0xef].into(),
            ..Default::default()
        };

        assert_eq!(
            BaseChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Insecure {
                rule: RuleId::R4_1SettingsChange,
            }
        );
    }

    #[tokio::test]
    async fn denies_delegatecall_to_unknown_target() {
        let transaction = SafeTransaction {
            safe: Address::new([1u8; 20]),
            to: Address::new([2u8; 20]),
            operation: Operation::DelegateCall,
            ..Default::default()
        };

        assert_eq!(
            BaseChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Insecure {
                rule: RuleId::R4_2DelegatecallIntegrity,
            }
        );
    }

    #[tokio::test]
    async fn affirms_to_and_operation_when_the_base_guarantees_hold() {
        let transaction = SafeTransaction {
            safe: Address::new([1u8; 20]),
            to: Address::new([2u8; 20]),
            ..Default::default()
        };

        assert_eq!(
            BaseChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Secure {
                coverage: Coverage::calls(1, AspectSet::TO | AspectSet::OPERATION),
            }
        );
    }

    /// The epic's headline fix: a batch's failing sub-call is cited under
    /// its own rule, not the container's R-4.2, even though the top-level
    /// transaction (the container the parser flattened) is itself a
    /// delegatecall.
    #[tokio::test]
    async fn cites_the_failing_call_s_own_rule_within_a_batch() {
        let safe = Address::new([1u8; 20]);
        let other = Address::new([2u8; 20]);
        let proposal = Proposal {
            transaction: SafeTransaction {
                safe,
                operation: Operation::DelegateCall,
                ..Default::default()
            },
            calls: vec![
                MetaTransaction {
                    to: other,
                    ..Default::default()
                },
                MetaTransaction {
                    to: safe,
                    data: vec![0xde, 0xad, 0xbe, 0xef].into(),
                    operation: Operation::Call,
                    ..Default::default()
                },
            ],
        };

        assert_eq!(
            BaseChecker.check(&proposal, &CheckContext::default()).await,
            Assessment::Insecure {
                rule: RuleId::R4_1SettingsChange,
            }
        );
    }

    #[test]
    fn allows_owner_change() {
        assert!(
            check(
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                U256::ZERO,
                hex("0xe318b52b\
                   0000000000000000000000002dc63c83040669f0adba5f832f713152ba862c97\
                   000000000000000000000000e7f8c378df23ebb06d5fc5a33bd471ef510f8cc9\
                   000000000000000000000000baf055b4ae60b897649f654df8def87bb4f86299"),
                Operation::Call,
            )
            .is_ok()
        );
    }

    #[test]
    fn allows_self_call_with_nonzero_value() {
        assert!(
            check(
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                U256::from(1u64),
                hex("0xe318b52b\
                  0000000000000000000000002dc63c83040669f0adba5f832f713152ba862c97\
                  000000000000000000000000e7f8c378df23ebb06d5fc5a33bd471ef510f8cc9\
                  000000000000000000000000baf055b4ae60b897649f654df8def87bb4f86299"),
                Operation::Call,
            )
            .is_ok()
        );
    }

    #[test]
    fn allows_cancellation_transaction() {
        assert!(
            check(
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                U256::ZERO,
                Bytes::new(),
                Operation::Call,
            )
            .is_ok()
        );
    }

    #[test]
    fn allows_singleton_upgrade() {
        assert!(
            check(
                address!("81a45AA50195f0A752159d5198780cDfb8e19732"),
                address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
                U256::ZERO,
                hex("0xed007fc6"),
                Operation::DelegateCall,
            )
            .is_ok()
        );
    }

    #[test]
    fn allows_delegate_call_with_nonzero_value() {
        assert!(
            check(
                address!("81a45AA50195f0A752159d5198780cDfb8e19732"),
                address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
                U256::from(1u64),
                hex("0xed007fc6"),
                Operation::DelegateCall,
            )
            .is_ok()
        );
    }

    #[test]
    fn denies_empty_self_delegatecall() {
        assert!(
            check(
                address!("1db92e2EeBC8E0c075a02BeA49a2935BcD2dFCF4"),
                address!("1db92e2EeBC8E0c075a02BeA49a2935BcD2dFCF4"),
                U256::ZERO,
                Bytes::new(),
                Operation::DelegateCall,
            )
            .is_err()
        );
    }

    #[test]
    fn denies_bybit_transaction() {
        assert!(
            check(
                address!("1db92e2EeBC8E0c075a02BeA49a2935BcD2dFCF4"),
                address!("96221423681A6d52E184D440a8eFCEbB105C7242"),
                U256::ZERO,
                hex("0xa9059cbb\
                   000000000000000000000000bdd077f651ebe7f7b3ce16fe5f2b025be2969516\
                   0000000000000000000000000000000000000000000000000000000000000000"),
                Operation::DelegateCall,
            )
            .is_err()
        );
    }

    #[test]
    fn denies_arbitrary_self_calls() {
        assert!(
            check(
                address!("3850cd76006dc6CaCBCBB514995C47Ca8Ad0bb96"),
                address!("A83c336B20401Af773B6219BA5027174338D1836"),
                U256::ZERO,
                hex("0x8d80ff0a0\
                   0000000000000000000000000000000000000000000000000000000000000200\
                   0000000000000000000000000000000000000000000000000000000000000790\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   00000000000000000000000000000000000000024610b5925000000000000000\
                   0000000005afe8f36504462aa6a7467372f9a41665820a14f00000000000000"),
                Operation::DelegateCall,
            )
            .is_err()
        );
    }

    #[test]
    fn allows_contract_deployment_via_create_call() {
        let safe = address!("8cf60b289f8d31f737049b590b5e4285ff0bd1d1");

        for create_call_addr in [
            address!("7cbB62EaA69F79e6873cD1ecB2392971036cFAa4"), // 1.3.0 - canonical
            address!("B19D6FFc2182150F8Eb585b79D4ABcd7C5640A9d"), // 1.3.0 - eip155
            address!("9b35Af71d77eaf8d7e40252370304687390A1A52"), // 1.4.1
            address!("2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4"), // 1.5.0
        ] {
            let data = Bytes::from(
                safe::performCreateCall {
                    value: U256::ZERO,
                    deploymentData: Bytes::from(vec![0x60, 0x00, 0x60, 0x00, 0xf3]),
                }
                .abi_encode(),
            );
            assert!(
                check(
                    safe,
                    create_call_addr,
                    U256::ZERO,
                    data,
                    Operation::DelegateCall
                )
                .is_ok(),
                "should allow performCreate delegatecall to {create_call_addr}",
            );

            let data = Bytes::from(
                safe::performCreate2Call {
                    value: U256::ZERO,
                    deploymentData: Bytes::from(vec![0x60, 0x00, 0x60, 0x00, 0xf3]),
                    salt: [0u8; 32].into(),
                }
                .abi_encode(),
            );
            assert!(
                check(
                    safe,
                    create_call_addr,
                    U256::ZERO,
                    data,
                    Operation::DelegateCall
                )
                .is_ok(),
                "should allow performCreate2 delegatecall to {create_call_addr}",
            );
        }
    }
}
