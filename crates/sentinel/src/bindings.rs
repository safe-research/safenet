use safenet_core::watcher_events;

pub mod oracle {
    use alloy::sol;

    sol! {
        // Mirrors `SentinelOracleRequest.State` in
        // `contracts/src/libraries/SentinelOracleRequests.sol`; `DisputeResolved.outcome` is
        // this type. Declaration order must match the Solidity enum exactly -- `sol!` decodes by
        // ordinal position, not by name. `NONE` is a zero-value sentinel the contract never
        // actually emits (it means "request was never created"), included here only to keep the
        // ordinals of every other variant aligned with the real enum.
        #[derive(Debug, PartialEq, Eq)]
        enum RequestState {
            NONE,
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
                address indexed sponsor,
                uint96 fee,
                uint96 bondTarget,
                uint96 slashAmount,
                uint64 commitDeadline,
                uint64 revealDeadline
            );
            event Committed(bytes32 indexed requestId, address indexed sentinel, uint96 bondAmount);
            event Revealed(
                bytes32 indexed requestId,
                address indexed sentinel,
                bool approved,
                uint96 bondAmount,
                string reason
            );
            event DisputeTriggered(bytes32 indexed requestId, uint64 deadline);
            event DisputeResolved(bytes32 indexed requestId, RequestState outcome, uint128 slashed, string context);
            event OracleResult(bytes32 indexed requestId, address indexed sponsor, bytes result, bool approved);
            event Claimed(bytes32 indexed requestId, address indexed sentinel, uint96 bondReturn, uint96 feeReward);

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
    use serde::{Serialize, Serializer, ser};

    sol! {
        #[derive(Debug, Default, PartialEq, Eq)]
        enum Operation { #[default] CALL, DELEGATECALL }

        // Full transaction struct carried by TransactionProposed; mirrors SafeTransaction.T.
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
        // keccak256("TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)")
        // Domain: { chainId, verifyingContract: consensus }
        #[derive(Debug)]
        struct TransactionProposal {
            uint64 epoch;
            address oracle;
            bytes oracleData;
            bytes32 safeTxHash;
        }

        #[derive(Debug)]
        contract Consensus {
            event TransactionProposed(
                bytes32 indexed safeTxHash,
                bytes32 indexed safeId,
                address indexed oracle,
                uint64 epoch,
                bytes oracleData,
                SafeTransaction transaction
            );
        }
    }

    impl Serialize for Operation {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let code = match self {
                Operation::CALL => 0,
                Operation::DELEGATECALL => 1,
                _ => return Err(ser::Error::custom("invalid Safe transaction operation")),
            };
            serializer.serialize_u8(code)
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
