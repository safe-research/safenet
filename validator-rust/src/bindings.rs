use alloy::sol;

sol! {
    #[derive(Debug)]
    struct Point {
        uint256 x;
        uint256 y;
    }

    #[derive(Debug)]
    struct Attestation {
        Point r;
        uint256 z;
    }

    #[derive(Debug)]
    struct KeyGenCommitment {
        Point q;
        Point[] c;
        Point r;
        uint256 mu;
    }

    #[derive(Debug)]
    struct SecretShare {
        Point y;
        uint256[] f;
    }

    #[derive(Debug)]
    struct NoncePair {
        Point d;
        Point e;
    }

    #[derive(Debug)]
    struct SafeTransaction {
        uint256 chainId;
        address safe;
        address to;
        uint256 value;
        bytes data;
        uint8 operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }

    #[sol(rpc)]
    #[derive(Debug)]
    contract Consensus {
        function getCoordinator() public view returns (address);

        event EpochStaged(
            uint64 indexed activeEpoch,
            uint64 indexed proposedEpoch,
            uint64 rolloverBlock,
            bytes32 groupId,
            Point groupKey,
            bytes32 signatureId,
            Attestation attestation
        );

        event TransactionProposed(
            bytes32 indexed safeTxHash,
            uint256 indexed chainId,
            address indexed safe,
            uint64 epoch,
            SafeTransaction transaction
        );

        event EpochProposed(
            uint64 indexed activeEpoch,
            uint64 indexed proposedEpoch,
            uint64 rolloverBlock,
            bytes32 groupId,
            Point groupKey
        );

        event TransactionAttested(
            bytes32 indexed safeTxHash,
            uint256 indexed chainId,
            address indexed safe,
            uint64 epoch,
            bytes32 signatureId,
            Attestation attestation
        );
    }

    #[derive(Debug)]
    contract Coordinator {

        event Sign(
            address indexed initiator,
            bytes32 indexed gid,
            bytes32 indexed message,
            bytes32 sid,
            uint64 sequence
        );


        event SignCompleted(
            bytes32 indexed sid,
            bytes32 indexed selectionRoot,
            Attestation signature
        );


        event KeyGen(
            bytes32 indexed gid,
            bytes32 participants,
            uint16 count,
            uint16 threshold,
            bytes32 indexed context
        );


        event KeyGenCommitted(
            bytes32 indexed gid,
            address participant,
            KeyGenCommitment commitment,
            bool committed
        );


        event KeyGenSecretShared(
            bytes32 indexed gid,
            address participant,
            SecretShare share,
            bool shared
        );


        event KeyGenConfirmed(
            bytes32 indexed gid,
            address participant,
            bool confirmed
        );


        event KeyGenComplained(
            bytes32 indexed gid,
            address plaintiff,
            address accused,
            bool compromised
        );


        event KeyGenComplaintResponded(
            bytes32 indexed gid,
            address plaintiff,
            address accused,
            uint256 secretShare
        );


        event Preprocess(
            bytes32 indexed gid,
            address participant,
            uint64 chunk,
            bytes32 commitment
        );


        event SignRevealedNonces(
            bytes32 indexed sid,
            address participant,
            NoncePair nonces
        );


        event SignShared(
            bytes32 indexed sid,
            bytes32 indexed selectionRoot,
            address participant,
            uint256 z
        );
    }
}
