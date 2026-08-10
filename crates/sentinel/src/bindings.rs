use safenet_core::watcher_events;

pub mod oracle {
    use alloy::sol;

    sol! {
        // Mirrors `SentinelOracleRequest.State` in
        // `contracts/src/libraries/SentinelOracleRequestsV2.sol`; `DisputeResolved.outcome` is
        // this type.
        #[derive(Debug, PartialEq, Eq)]
        enum RequestState {
            PENDING,
            FROZEN,
            RESOLVED_APPROVED,
            RESOLVED_DENIED,
            TIMED_OUT
        }

        #[derive(Debug)]
        contract SentinelOracle {
            event NewRequest(
                bytes32 indexed requestId,
                address indexed proposer,
                uint256 fee,
                uint256 bondTarget,
                uint256 commitDeadline,
                uint256 revealDeadline
            );
            event Committed(bytes32 indexed requestId, address indexed sentinel, uint256 bondAmount);
            event Revealed(
                bytes32 indexed requestId,
                address indexed sentinel,
                bool approved,
                uint256 bondAmount,
                string reason
            );
            event DisputeResolved(bytes32 indexed requestId, RequestState outcome, uint256 slashed, string context);

            function commit(bytes32 requestId, bytes32 commitHash) external;
            function reveal(bytes32 requestId, bool approve, bytes32 salt, string calldata reason) external;
            function hashCommitment(
                address sentinel,
                bytes32 requestId,
                bool approve,
                bytes32 salt,
                string calldata reason
            ) external pure returns (bytes32);
            function finalize(bytes32 requestId) external;
            function claim(bytes32 requestId) external;
        }

        #[derive(Debug)]
        contract ERC20 {
            function approve(address spender, uint256 amount) external returns (bool);
            function allowance(address owner, address spender) external view returns (uint256);
        }
    }
}

pub mod consensus {
    use alloy::sol;
    use serde::Serialize;

    sol! {
        #[derive(Debug, Default, PartialEq, Eq, Serialize)]
        enum Operation { #[default] CALL, DELEGATECALL }

        // Full transaction struct carried by OracleTransactionProposed; mirrors SafeTransaction.T.
        #[derive(Debug, Default, PartialEq, Eq, Serialize)]
        struct SafeTransaction {
            uint256 chainId;
            address safe;
            address to;
            uint256 value;
            bytes data;
            Operation operation;
            uint256 safeTxGas;
            uint256 baseGas;
            uint256 gasPrice;
            address gasToken;
            address refundReceiver;
            uint256 nonce;
        }

        // EIP-712 struct for the oracle requestId.
        // Field order and types must exactly match the onchain typehash in ConsensusMessages.sol:
        // keccak256("OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash)")
        // Domain: { chainId, verifyingContract: consensus }
        #[derive(Debug)]
        struct OracleTransactionProposal {
            uint64 epoch;
            address oracle;
            bytes32 safeTxHash;
        }

        #[derive(Debug)]
        contract Consensus {
            event OracleTransactionProposed(
                bytes32 indexed safeTxHash,
                uint256 indexed chainId,
                address indexed safe,
                uint64 epoch,
                address oracle,
                SafeTransaction transaction
            );
        }
    }

    // `sol!` requires custom types used as event fields to be declared in
    // the same macro invocation, so this ABI-decoding copy can't be replaced
    // with the plain `safe_tx::{SafeTransaction, Operation}` types.
    // `TryFrom` converts it at the point an event is consumed, because the
    // generated operation enum has an `__Invalid` value the policy type
    // deliberately cannot represent.
    //
    // TODO: this same event-binding + `TryFrom` conversion is duplicated
    // near-verbatim in `crates/validator/src/bindings.rs`. Consider moving
    // the ABI/event definitions into a shared crate so both consumers
    // declare the `sol!` types (and this conversion) exactly once.
    impl TryFrom<SafeTransaction> for safe_tx::SafeTransaction {
        type Error = Operation;

        fn try_from(tx: SafeTransaction) -> Result<Self, Self::Error> {
            let operation = match tx.operation {
                Operation::CALL => safe_tx::Operation::CALL,
                Operation::DELEGATECALL => safe_tx::Operation::DELEGATECALL,
                Operation::__Invalid => return Err(Operation::__Invalid),
            };
            Ok(safe_tx::SafeTransaction {
                chainId: tx.chainId,
                safe: tx.safe,
                to: tx.to,
                value: tx.value,
                data: tx.data,
                operation,
                safeTxGas: tx.safeTxGas,
                baseGas: tx.baseGas,
                gasPrice: tx.gasPrice,
                gasToken: tx.gasToken,
                refundReceiver: tx.refundReceiver,
                nonce: tx.nonce,
            })
        }
    }

    impl From<safe_tx::SafeTransaction> for SafeTransaction {
        fn from(tx: safe_tx::SafeTransaction) -> Self {
            let operation = match tx.operation {
                safe_tx::Operation::CALL => Operation::CALL,
                safe_tx::Operation::DELEGATECALL => Operation::DELEGATECALL,
            };
            Self {
                chainId: tx.chainId,
                safe: tx.safe,
                to: tx.to,
                value: tx.value,
                data: tx.data,
                operation,
                safeTxGas: tx.safeTxGas,
                baseGas: tx.baseGas,
                gasPrice: tx.gasPrice,
                gasToken: tx.gasToken,
                refundReceiver: tx.refundReceiver,
                nonce: tx.nonce,
            }
        }
    }
}

// Safe EIP-712 signing type, separate from the Safenet contract ABI bindings above.
pub mod safe {
    use alloy::sol;

    sol! {
        #[derive(Debug)]
        enum Operation { CALL, DELEGATECALL }

        // EIP-712 struct for the Safe transaction struct hash.
        // Field order and types must exactly match the onchain typehash in SafeTransaction.sol:
        // keccak256("SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,
        //   uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)")
        // Domain: { chainId, verifyingContract: safe }
        #[derive(Debug)]
        struct SafeTx {
            address to;
            uint256 value;
            bytes data;
            Operation operation;
            uint256 safeTxGas;
            uint256 baseGas;
            uint256 gasPrice;
            address gasToken;
            address refundReceiver;
            uint256 nonce;
        }
    }
}

// The event set consumed by the Watcher and StateMachine: all events from
// both the SentinelOracle and Consensus contracts.
watcher_events! {
    #[derive(Debug)]
    pub enum SentinelEvents {
        Oracle(oracle::SentinelOracle::SentinelOracleEvents),
        Consensus(consensus::Consensus::ConsensusEvents),
    }
}
