mod multi_send;

use self::multi_send::{MultiSendVersion, decode_multi_send};
use crate::bindings::{Operation, SafeTransaction};
use alloy::{
    primitives::{Address, address},
    sol_types::SolCall as _,
};

mod bindings {
    alloy::sol! {
        function addOwnerWithThreshold(address owner, uint256 threshold);
        function removeOwner(address prevOwner, address owner, uint256 threshold);
        function swapOwner(address prevOwner, address oldOwner, address newOwner);
        function changeThreshold(uint256 threshold);
        function setFallbackHandler(address handler);
        function setGuard(address guard);
        function enableModule(address module);
        function disableModule(address prevModule, address module);
        function setModuleGuard(address guard);
        function migrateSingleton();
        function migrateWithFallbackHandler();
        function migrateL2Singleton();
        function migrateL2WithFallbackHandler();
        function signMessage(bytes message);
        function multiSend(bytes transactions);
        function performCreate(uint256 value, bytes deploymentData);
        function performCreate2(uint256 value, bytes deploymentData, bytes32 salt);
    }
}

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

/// Returns `true` if a proposed Safe transaction is allowed by Safenet policy.
pub fn check_transaction(tx: &SafeTransaction) -> bool {
    check_calls(tx) || check_delegate_calls(tx) || check_multi_send(tx)
}

/// Calls to other contracts are freely allowed; self-calls are restricted to a
/// whitelist of Safe management functions.
fn check_calls(tx: &SafeTransaction) -> bool {
    if tx.operation != Operation::CALL {
        return false;
    }
    if tx.safe != tx.to {
        return true;
    }
    // receive: empty calldata with any value (e.g. cancellation transactions).
    if tx.data.is_empty() {
        return true;
    }
    check_self_calls(tx)
}

/// Checks that a self-call targets one of the allowed Safe management
/// functions (with argument validation where necessary).
fn check_self_calls(tx: &SafeTransaction) -> bool {
    // No-arg checks: any calldata starting with the right selector is allowed.
    if tx
        .data
        .starts_with(&bindings::addOwnerWithThresholdCall::SELECTOR)
    {
        return true;
    }
    if tx.data.starts_with(&bindings::removeOwnerCall::SELECTOR) {
        return true;
    }
    if tx.data.starts_with(&bindings::swapOwnerCall::SELECTOR) {
        return true;
    }
    if tx
        .data
        .starts_with(&bindings::changeThresholdCall::SELECTOR)
    {
        return true;
    }
    if tx.data.starts_with(&bindings::disableModuleCall::SELECTOR) {
        return true;
    }

    // Arg-validated checks: the first address argument must be in the allow-list.
    if tx
        .data
        .starts_with(&bindings::setFallbackHandlerCall::SELECTOR)
    {
        return bindings::setFallbackHandlerCall::abi_decode(&tx.data)
            .ok()
            .is_some_and(|call| SUPPORTED_FALLBACK_HANDLERS.contains(&call.handler));
    }
    if tx.data.starts_with(&bindings::setGuardCall::SELECTOR) {
        return bindings::setGuardCall::abi_decode(&tx.data)
            .ok()
            .is_some_and(|call| SUPPORTED_GUARDS.contains(&call.guard));
    }
    if tx.data.starts_with(&bindings::enableModuleCall::SELECTOR) {
        return bindings::enableModuleCall::abi_decode(&tx.data)
            .ok()
            .is_some_and(|call| SUPPORTED_MODULES.contains(&call.module));
    }
    if tx.data.starts_with(&bindings::setModuleGuardCall::SELECTOR) {
        return bindings::setModuleGuardCall::abi_decode(&tx.data)
            .ok()
            .is_some_and(|call| SUPPORTED_MODULE_GUARDS.contains(&call.guard));
    }

    false
}

/// Delegate calls are restricted to known Safe migration and signing-library
/// contracts, each with a fixed set of allowed function selectors.
fn check_delegate_calls(tx: &SafeTransaction) -> bool {
    if tx.operation != Operation::DELEGATECALL {
        return false;
    }

    const MIGRATION_CONTRACTS: &[Address] = &[
        address!("6439e7ABD8Bb915A5263094784C5CF561c4172AC"),
        address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
    ];
    if MIGRATION_CONTRACTS.contains(&tx.to) {
        return tx
            .data
            .starts_with(&bindings::migrateSingletonCall::SELECTOR)
            || tx
                .data
                .starts_with(&bindings::migrateWithFallbackHandlerCall::SELECTOR)
            || tx
                .data
                .starts_with(&bindings::migrateL2SingletonCall::SELECTOR)
            || tx
                .data
                .starts_with(&bindings::migrateL2WithFallbackHandlerCall::SELECTOR);
    }

    const SIGN_MESSAGE_LIBS: &[Address] = &[
        address!("A65387F16B013cf2Af4605Ad8aA5ec25a2cbA3a2"),
        address!("98FFBBF51bb33A056B08ddf711f289936AafF717"),
        address!("d53cd0aB83D845Ac265BE939c57F53AD838012c9"),
        address!("4FfeF8222648872B3dE295Ba1e49110E61f5b5aa"),
    ];
    if SIGN_MESSAGE_LIBS.contains(&tx.to) {
        return tx.data.starts_with(&bindings::signMessageCall::SELECTOR);
    }

    const CREATE_CALL_CONTRACTS: &[Address] = &[
        address!("7cbB62EaA69F79e6873cD1ecB2392971036cFAa4"), // 1.3.0 - canonical
        address!("B19D6FFc2182150F8Eb585b79D4ABcd7C5640A9d"), // 1.3.0 - eip155
        address!("9b35Af71d77eaf8d7e40252370304687390A1A52"), // 1.4.1
        address!("2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4"), // 1.5.0
    ];
    if CREATE_CALL_CONTRACTS.contains(&tx.to) {
        return tx.data.starts_with(&bindings::performCreateCall::SELECTOR)
            || tx.data.starts_with(&bindings::performCreate2Call::SELECTOR);
    }

    false
}

/// Delegate calls to known multi-send contracts are allowed when each packed
/// sub-transaction passes the appropriate check.
fn check_multi_send(tx: &SafeTransaction) -> bool {
    if tx.operation != Operation::DELEGATECALL {
        return false;
    }
    let Ok(call) = bindings::multiSendCall::abi_decode(&tx.data) else {
        return false;
    };

    let (allows_delegate_calls, version) = match tx.to {
        a if a == address!("218543288004CD07832472D464648173c77D7eB7") => {
            (true, MultiSendVersion::V150Plus)
        }
        a if a == address!("A83c336B20401Af773B6219BA5027174338D1836") => {
            (false, MultiSendVersion::V150Plus)
        }
        a if a == address!("38869bf66a61cF6bDB996A6aE40D5853Fd43B526") => {
            (true, MultiSendVersion::Legacy)
        }
        a if a == address!("9641d764fc13c8B624c04430C7356C1C7C8102e2") => {
            (false, MultiSendVersion::Legacy)
        }
        a if a == address!("A238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761") => {
            (true, MultiSendVersion::Legacy)
        }
        a if a == address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D") => {
            (false, MultiSendVersion::Legacy)
        }
        a if a == address!("998739BFdAAdde7C933B942a68053933098f9EDa") => {
            (true, MultiSendVersion::Legacy)
        }
        a if a == address!("A1dabEF33b3B82c7814B6D82A79e50F4AC44102B") => {
            (false, MultiSendVersion::Legacy)
        }
        _ => return false,
    };

    let Some(sub_txs) = decode_multi_send(tx.safe, &call.transactions, version) else {
        return false;
    };

    sub_txs.iter().all(|sub_tx| {
        check_calls(sub_tx) || (allows_delegate_calls && check_delegate_calls(sub_tx))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{Address, Bytes, U256, address},
        sol,
    };

    sol! {
        function approve(address spender, uint256 amount);
    }

    fn tx(
        safe: Address,
        to: Address,
        value: U256,
        data: impl Into<Bytes>,
        operation: Operation,
    ) -> SafeTransaction {
        SafeTransaction {
            chainId: U256::ZERO,
            safe,
            to,
            value,
            data: data.into(),
            operation,
            safeTxGas: U256::ZERO,
            baseGas: U256::ZERO,
            gasPrice: U256::ZERO,
            gasToken: Address::ZERO,
            refundReceiver: Address::ZERO,
            nonce: U256::ZERO,
        }
    }

    fn hex(s: &str) -> Bytes {
        let s = s.strip_prefix("0x").unwrap_or(s);
        Bytes::from(alloy::primitives::hex::decode(s).expect("invalid hex"))
    }

    /// Packs a sub-transaction into the multi-send wire format.
    fn pack(operation: Operation, to: Address, value: U256, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(operation as u8);
        out.extend_from_slice(to.as_slice());
        out.extend_from_slice(&value.to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(data.len()).to_be_bytes::<32>());
        out.extend_from_slice(data);
        out
    }

    /// Encodes a `multiSend(bytes)` call wrapping the given packed sub-txs.
    fn multisend(sub_txs: &[Vec<u8>]) -> Bytes {
        let transactions: Vec<u8> = sub_txs.iter().flatten().cloned().collect();
        Bytes::from(
            bindings::multiSendCall {
                transactions: Bytes::from(transactions),
            }
            .abi_encode(),
        )
    }

    #[test]
    fn allows_owner_change() {
        assert!(check_transaction(&tx(
            address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            U256::ZERO,
            hex("0xe318b52b\
                   0000000000000000000000002dc63c83040669f0adba5f832f713152ba862c97\
                   000000000000000000000000e7f8c378df23ebb06d5fc5a33bd471ef510f8cc9\
                   000000000000000000000000baf055b4ae60b897649f654df8def87bb4f86299"),
            Operation::CALL,
        )));
    }

    #[test]
    fn allows_self_call_with_nonzero_value() {
        assert!(check_transaction(&tx(
            address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            U256::from(1u64),
            hex("0xe318b52b\
                  0000000000000000000000002dc63c83040669f0adba5f832f713152ba862c97\
                  000000000000000000000000e7f8c378df23ebb06d5fc5a33bd471ef510f8cc9\
                  000000000000000000000000baf055b4ae60b897649f654df8def87bb4f86299"),
            Operation::CALL,
        )));
    }

    #[test]
    fn allows_multisend_with_calls_to_other_contracts() {
        assert!(check_transaction(&tx(
            address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D"),
            U256::ZERO,
            hex("0x8d80ff0a\
                   0000000000000000000000000000000000000000000000000000000000000020\
                   000000000000000000000000000000000000000000000000000000000000046b\
                   00469788fe6e9e9681c6ebf3bf78e7fd26fc0154460000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   000000000000000000000000000000000000000044bd86e508736166652e6574\
                   6800000000000000000000000000000000000000000000000000000000000000\
                   0000000000d101ee8ab789b5ac467cd0c5343ac596e074e7a900a0b937d5c8e3\
                   2a80e3a8ed4227cd020221544ee6000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   00000000000000000000000002c4bf6213e40000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   0000000000000000000000000000000001a00000000000000000000000000000\
                   00000000000000000000000000005bacaa200000000000000000000000000000\
                   00000000000000000017097071d7b56600000000000000000000000000000000\
                   0000000000000000000000000000000000a00000000000000000000000000000\
                   0000000000000000000000000000000000109203d17922f539fb574e5e1f6d90\
                   465148bd7db15545841b8f3b4abe9df5d040e7dab6bf83b5e440418b27e82fcc\
                   724f0ff2dba673de766816c49b4c929b00c327b5754ca9c08cadb9ffdfe5a520\
                   7796d9be48d869dd0c500515774c0747e0087ffeb9bfa892a609ef0e4d8e5690\
                   6cc226ebcdeed2e0956a1f8a510df63fa4704fa0478c551520b05243f319f13c\
                   8b0b2cd9fb5f6d2167906639eed96af45357c66521c5276873caaec751f783a1\
                   f5b8bf1a4721b6edddbe6f1c6461415f139915eb66ee9f44732b9861438a746c\
                   644967878a71a6242baa91d4487be75fb4cf0a6d3701dcb0b97559583b1aa242\
                   1882d56318b02c6c007abd359479775873df9fea2b42423550759e89e535b4d4\
                   e161e8fb2d863e5b60c01358a14197089be5b56c4486661ebec5747ec78d3ffa\
                   c59923102c6a5d54c5defa08537873b7c82435fe35c5adc5c483754e6a5a344a\
                   e2ebdfa8c45aeaa13f577077674a91a947638b4fca97dbd562b4bce9c318eb9b\
                   985ea30aebe29cae941811e93425cc14762280ac2a9f69aebd287a893dc0efc0\
                   b5021b8ee3669877d200138fbf8fd4bb71fb64c7556587b8b27dd506cc48f666\
                   7566a8899de7a16e64f8c9c4c931bce8ebddc5d2460186f7233c927e7db2dcc7\
                   03c0e500b653ca82273b7bfad8045d85a470260ff1ff6ec89ea0b57028c682d3\
                   d1bdd86c9f393863a04e52b4b82ca363cbf700a0b937d5c8e32a80e3a8ed4227\
                   cd020221544ee600000000000000000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   000000000000640087b83f5a5721848b2fa4cf4d2b8e3c7df9882952036c5ed2\
                   4d31f5b1c71352252dcc93000000000000000000000000f01888f0677547ec07\
                   cd16c8680e699c96588e6b00000000000000000000000000000000ffffffffff\
                   ffffffffffffffffffffff000000000000000000000000000000000000000000"),
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn allows_cancellation_transaction() {
        assert!(check_transaction(&tx(
            address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            U256::ZERO,
            Bytes::new(),
            Operation::CALL,
        )));
    }

    #[test]
    fn allows_multisend_with_multiple_owner_changes() {
        assert!(check_transaction(&tx(
            address!("81a45AA50195f0A752159d5198780cDfb8e19732"),
            address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D"),
            U256::ZERO,
            hex("0x8d80ff0a\
                   0000000000000000000000000000000000000000000000000000000000000020\
                   000000000000000000000000000000000000000000000000000000000000022b\
                   0081a45aa50195f0a752159d5198780cdfb8e197320000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   000000000000000000000000000000000000000064e318b52b00000000000000\
                   00000000003242071b0b406b6661af2de1115cd46567ab091700000000000000\
                   000000000007e5069f8f8e6a80432b13f20e9d4906de097e1a00000000000000\
                   0000000000d80e356e94fb3f8e85b39b0c730fb7152e8cbd800081a45aa50195\
                   f0a752159d5198780cdfb8e19732000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   0000000000000000000000000064f8dc5dd9000000000000000000000000d80e\
                   356e94fb3f8e85b39b0c730fb7152e8cbd800000000000000000000000000709\
                   41a8e2d7289e9594798f65a9379f6828d5bb0000000000000000000000000000\
                   0000000000000000000000000000000000020081a45aa50195f0a752159d5198\
                   780cdfb8e1973200000000000000000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   00000000000064f8dc5dd90000000000000000000000005afe8f36504462aa6a\
                   7467372f9a41665820a14f000000000000000000000000c0ffeee8baafa7ba6a\
                   6af2329892b88796cf44cf000000000000000000000000000000000000000000\
                   0000000000000000000002000000000000000000000000000000000000000000"),
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn allows_singleton_upgrade() {
        assert!(check_transaction(&tx(
            address!("81a45AA50195f0A752159d5198780cDfb8e19732"),
            address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
            U256::ZERO,
            hex("0xed007fc6"),
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn allows_delegate_call_with_nonzero_value() {
        assert!(check_transaction(&tx(
            address!("81a45AA50195f0A752159d5198780cDfb8e19732"),
            address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
            U256::from(1u64),
            hex("0xed007fc6"),
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn denies_empty_self_delegatecall() {
        assert!(!check_transaction(&tx(
            address!("1db92e2EeBC8E0c075a02BeA49a2935BcD2dFCF4"),
            address!("1db92e2EeBC8E0c075a02BeA49a2935BcD2dFCF4"),
            U256::ZERO,
            Bytes::new(),
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn denies_bybit_transaction() {
        assert!(!check_transaction(&tx(
            address!("1db92e2EeBC8E0c075a02BeA49a2935BcD2dFCF4"),
            address!("96221423681A6d52E184D440a8eFCEbB105C7242"),
            U256::ZERO,
            hex("0xa9059cbb\
                   000000000000000000000000bdd077f651ebe7f7b3ce16fe5f2b025be2969516\
                   0000000000000000000000000000000000000000000000000000000000000000"),
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn denies_arbitrary_self_calls() {
        assert!(!check_transaction(&tx(
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
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn allows_multisend_with_valid_delegatecall() {
        let safe = address!("3850cd76006dc6CaCBCBB514995C47Ca8Ad0bb96");

        let sign_msg_data = bindings::signMessageCall {
            message: Bytes::from("swap GNO for SAFE"),
        }
        .abi_encode();

        let approve_data = approveCall {
            spender: address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
            amount: U256::from(1_000_000_000_000_000_000_000_u128),
        }
        .abi_encode();

        let data = multisend(&[
            pack(
                Operation::DELEGATECALL,
                address!("4FfeF8222648872B3dE295Ba1e49110E61f5b5aa"),
                U256::ZERO,
                &sign_msg_data,
            ),
            pack(
                Operation::CALL,
                address!("9C58BAcC331c9aa871AFD802DB6379a98e80CEdb"),
                U256::ZERO,
                &approve_data,
            ),
        ]);

        assert!(check_transaction(&tx(
            safe,
            address!("218543288004CD07832472D464648173c77D7eB7"),
            U256::ZERO,
            data,
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn denies_delegatecall_to_callonly_multisend() {
        let safe = address!("3850cd76006dc6CaCBCBB514995C47Ca8Ad0bb96");

        let sign_msg_data = bindings::signMessageCall {
            message: Bytes::from("swap GNO for SAFE"),
        }
        .abi_encode();

        let approve_data = approveCall {
            spender: address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
            amount: U256::from(1_000_000_000_000_000_000_000_u128),
        }
        .abi_encode();

        let data = multisend(&[
            pack(
                Operation::DELEGATECALL,
                address!("4FfeF8222648872B3dE295Ba1e49110E61f5b5aa"),
                U256::ZERO,
                &sign_msg_data,
            ),
            pack(
                Operation::CALL,
                address!("9C58BAcC331c9aa871AFD802DB6379a98e80CEdb"),
                U256::ZERO,
                &approve_data,
            ),
        ]);

        // Same data but to the call-only multisend — delegate calls not allowed.
        assert!(!check_transaction(&tx(
            safe,
            address!("A83c336B20401Af773B6219BA5027174338D1836"),
            U256::ZERO,
            data,
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn allows_multisend_where_last_sub_tx_has_no_data() {
        let safe = address!("3850cd76006dc6CaCBCBB514995C47Ca8Ad0bb96");
        let recipient = address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110");

        let data = multisend(&[pack(Operation::CALL, recipient, U256::from(1u64), &[])]);
        assert!(check_transaction(&tx(
            safe,
            address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D"),
            U256::ZERO,
            data,
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn allows_empty_multisend() {
        let safe = address!("3850cd76006dc6CaCBCBB514995C47Ca8Ad0bb96");
        assert!(check_transaction(&tx(
            safe,
            address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D"),
            U256::ZERO,
            multisend(&[]),
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn allows_multisend_with_nonzero_value() {
        let safe = address!("3850cd76006dc6CaCBCBB514995C47Ca8Ad0bb96");
        let recipient = address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110");

        let data = multisend(&[pack(Operation::CALL, recipient, U256::from(1u64), &[])]);
        assert!(check_transaction(&tx(
            safe,
            address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D"),
            U256::from(1u64),
            data,
            Operation::DELEGATECALL,
        )));
    }

    #[test]
    fn denies_delegatecall_via_multisend_to_disallowed_target() {
        let safe = address!("1db92e2EeBC8E0c075a02BeA49a2935BcD2dFCF4");
        // Multisend containing a delegatecall to a non-whitelisted address.
        let data = hex("0x8d80ff0a\
               0000000000000000000000000000000000000000000000000000000000000020\
               0000000000000000000000000000000000000000000000000000000000000099\
               0196221423681A6d52E184D440a8eFCEbB105C72420000000000000000000000\
               0000000000000000000000000000000000000000000000000000000000000000\
               000000000000000000000000000000000000000044a9059cbb00000000000000\
               0000000000bdd077f651ebe7f7b3ce16fe5f2b025be296951600000000000000\
               0000000000000000000000000000000000000000000000000000000000000000");

        // All multisend contracts that allow delegate calls should still deny
        // this because the target is not on the delegate-call allow-list.
        for multisend_addr in [
            address!("218543288004CD07832472D464648173c77D7eB7"),
            address!("38869bf66a61cF6bDB996A6aE40D5853Fd43B526"),
            address!("A238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761"),
            address!("998739BFdAAdde7C933B942a68053933098f9EDa"),
        ] {
            assert!(
                !check_transaction(&tx(
                    safe,
                    multisend_addr,
                    U256::ZERO,
                    data.clone(),
                    Operation::DELEGATECALL
                )),
                "multisend at {multisend_addr} should deny delegatecall to disallowed target",
            );
        }
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
                bindings::performCreateCall {
                    value: U256::ZERO,
                    deploymentData: Bytes::from(vec![0x60, 0x00, 0x60, 0x00, 0xf3]),
                }
                .abi_encode(),
            );
            assert!(
                check_transaction(&tx(
                    safe,
                    create_call_addr,
                    U256::ZERO,
                    data,
                    Operation::DELEGATECALL
                )),
                "should allow performCreate delegatecall to {create_call_addr}",
            );

            let data = Bytes::from(
                bindings::performCreate2Call {
                    value: U256::ZERO,
                    deploymentData: Bytes::from(vec![0x60, 0x00, 0x60, 0x00, 0xf3]),
                    salt: [0u8; 32].into(),
                }
                .abi_encode(),
            );
            assert!(
                check_transaction(&tx(
                    safe,
                    create_call_addr,
                    U256::ZERO,
                    data,
                    Operation::DELEGATECALL
                )),
                "should allow performCreate2 delegatecall to {create_call_addr}",
            );
        }
    }
}
