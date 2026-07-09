use safenet_core::watcher_events;

pub mod oracle {
    use alloy::sol;

    sol! {
        // Mirrors `SentinelOracleRequests.ResolveReason` in
        // `contracts/src/libraries/SentinelOracleRequests.sol`; `OracleResult.result` is
        // `abi.encode`d as this type.
        #[derive(Debug, PartialEq, Eq)]
        enum ResolveReason {
            UNANIMOUS_APPROVE,
            UNANIMOUS_DENY,
            TIMEOUT,
            ARBITRATION
        }

        #[derive(Debug)]
        contract SentinelOracle {
            event NewRequest(
                bytes32 indexed requestId,
                address indexed proposer,
                uint256 fee,
                uint256 bondTarget,
                uint256 deadline
            );
            event Committed(
                bytes32 indexed requestId,
                address indexed sentinel,
                bool approved,
                uint256 bondAmount,
                uint256 position
            );
            event OracleResult(
                bytes32 indexed requestId,
                address indexed proposer,
                bytes result,
                bool approved
            );

            function commitApprove(bytes32 requestId) external;
            function commitDeny(bytes32 requestId) external;
            function finalize(bytes32 requestId) external;
            function claim(bytes32 requestId) external;
        }

        #[derive(Debug)]
        contract SentinelOracleV2 {
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
                uint256 bondAmount
            );
            event OracleResult(
                bytes32 indexed requestId,
                address indexed proposer,
                bytes result,
                bool approved
            );

            function commit(bytes32 requestId, bytes32 commitHash) external;
            function reveal(bytes32 requestId, bool approve, bytes32 salt) external;
            function hashCommitment(address sentinel, bytes32 requestId, bool approve, bytes32 salt)
                external
                pure
                returns (bytes32);
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

    sol! {
        #[derive(Debug, Default)]
        enum Operation { #[default] CALL, DELEGATECALL }

        // Full transaction struct carried by OracleTransactionProposed; mirrors SafeTransaction.T.
        #[derive(Debug, Default)]
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
