//! `sol!`-generated bindings for the onchain contracts the validator interacts
//! with including the [`ValidatorEvents`] set the state machine consumes.
//!
//! Function bindings only need their calldata (the `*Call` types), so return
//! values are transcribed only where a return is actually decoded onchain
//! (`getValidatorStaker`); the mutating calls omit them.

use alloy::sol;
use serde::{Deserialize, Serialize};

sol! {
    /// A secp256k1 point in affine (uncompressed) coordinates, as encoded
    /// onchain (`Secp256k1.Point`).
    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Point {
        uint256 x;
        uint256 y;
    }

    /// A FROST group signature (`FROST.Signature`): the group commitment point
    /// `r` and the scalar component `z`. Onchain attestations and completed
    /// signatures share this shape.
    #[derive(Debug, Default)]
    struct Signature {
        Point r;
        uint256 z;
    }

    /// Safe transaction operation type; mirrors `Enum.Operation` onchain.
    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    enum Operation {
        #[default]
        CALL,
        DELEGATECALL,
    }

    /// A full Safe transaction as carried by the `(Oracle)TransactionProposed`
    /// events (the 12-field `SafeTransaction.T` tuple).
    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

    /// DKG commitment published in `keyGenAndCommit` / `keyGenCommit`: the
    /// public encryption key `q`, the commitment vector `c`, the proof-of-
    /// knowledge nonce `r` and its scalar `mu`.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct KeyGenCommitment {
        Point q;
        Point[] c;
        Point r;
        uint256 mu;
    }

    /// DKG secret share published in `keyGenSecretShare`: the participant public
    /// key share `y` and the polynomial evaluations `f` encrypted for peers.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct KeyGenSecretShare {
        Point y;
        uint256[] f;
    }

    /// A revealed FROST nonce commitment pair (hiding `d`, binding `e`).
    #[derive(Debug, Default, PartialEq, Eq)]
    struct SignNonces {
        Point d;
        Point e;
    }

    /// The signer-set selection accompanying a signature share: the group
    /// commitment point `r` and the merkle `root` of the selected participants.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct SignSelection {
        Point r;
        bytes32 root;
    }

    /// A single participant's FROST signature share (`FROST.SignatureShare`):
    /// the participant commitment `r`, the scalar share `z` and the Lagrange
    /// coefficient `l`.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct SignatureShare {
        Point r;
        uint256 z;
        uint256 l;
    }

    /// Callback target and context for the `*WithCallback` coordinator calls.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Callback {
        address target;
        bytes context;
    }

    #[sol(rpc)]
    #[derive(Debug)]
    contract Consensus {
        event EpochProposed(
            uint64 indexed activeEpoch,
            uint64 indexed proposedEpoch,
            uint64 rolloverBlock,
            bytes32 groupId,
            Point groupKey
        );
        event EpochStaged(
            uint64 indexed activeEpoch,
            uint64 indexed proposedEpoch,
            uint64 rolloverBlock,
            bytes32 groupId,
            Point groupKey,
            bytes32 signatureId,
            Signature attestation
        );
        event TransactionProposed(
            bytes32 indexed safeTxHash,
            uint256 indexed chainId,
            address indexed safe,
            uint64 epoch,
            SafeTransaction transaction
        );
        event TransactionAttested(
            bytes32 indexed safeTxHash,
            uint256 indexed chainId,
            address indexed safe,
            uint64 epoch,
            bytes32 signatureId,
            Signature attestation
        );
        event OracleTransactionProposed(
            bytes32 indexed safeTxHash,
            uint256 indexed chainId,
            address indexed safe,
            uint64 epoch,
            address oracle,
            SafeTransaction transaction
        );
        event OracleTransactionAttested(
            bytes32 indexed safeTxHash,
            uint256 indexed chainId,
            address indexed safe,
            uint64 epoch,
            address oracle,
            bytes32 signatureId,
            Signature attestation
        );
        event ValidatorStakerSet(address indexed validator, address staker);

        function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 groupId) external;
        function stageEpoch(
            uint64 proposedEpoch,
            uint64 rolloverBlock,
            bytes32 groupId,
            bytes32 signatureId
        ) external;
        function attestTransaction(
            uint64 epoch,
            uint256 chainId,
            address safe,
            bytes32 safeTxStructHash,
            bytes32 signatureId
        ) external;
        function attestOracleTransaction(
            uint64 epoch,
            address oracle,
            uint256 chainId,
            address safe,
            bytes32 safeTxStructHash,
            bytes32 signatureId
        ) external;
        function setValidatorStaker(address staker) external;
        function getValidatorStaker(address validator) external view returns (address staker);
        function getCoordinator() external view returns (address coordinator);
    }

    #[sol(rpc)]
    #[derive(Debug)]
    contract Coordinator {
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
            KeyGenSecretShare share,
            bool shared
        );
        event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed);
        event KeyGenComplained(bytes32 indexed gid, address plaintiff, address accused, bool compromised);
        event KeyGenComplaintResponded(
            bytes32 indexed gid,
            address plaintiff,
            address accused,
            uint256 secretShare
        );
        event Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment);
        event Sign(
            address indexed initiator,
            bytes32 indexed gid,
            bytes32 indexed message,
            bytes32 sid,
            uint64 sequence
        );
        event SignRevealedNonces(bytes32 indexed sid, address participant, SignNonces nonces);
        event SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, address participant, uint256 z);
        event SignCompleted(bytes32 indexed sid, bytes32 indexed selectionRoot, Signature signature);

        function keyGenAndCommit(
            bytes32 participants,
            uint16 count,
            uint16 threshold,
            bytes32 context,
            bytes32[] poap,
            KeyGenCommitment commitment
        ) external;
        function keyGenCommit(bytes32 gid, bytes32[] poap, KeyGenCommitment commitment) external;
        function keyGenSecretShare(bytes32 gid, KeyGenSecretShare share) external;
        function keyGenComplain(bytes32 gid, address accused) external;
        function keyGenComplaintResponse(bytes32 gid, address plaintiff, uint256 secretShare) external;
        function keyGenConfirm(bytes32 gid) external;
        function keyGenConfirmWithCallback(bytes32 gid, Callback callback) external;
        function preprocess(bytes32 gid, bytes32 commitment) external;
        function sign(bytes32 gid, bytes32 message) external;
        function signDecline(bytes32 sid) external;
        function signRevealNonces(bytes32 sid, SignNonces nonces, bytes32[] proof) external;
        function signShare(
            bytes32 sid,
            SignSelection selection,
            SignatureShare share,
            bytes32[] proof
        ) external;
        function signShareWithCallback(
            bytes32 sid,
            SignSelection selection,
            SignatureShare share,
            bytes32[] proof,
            Callback callback
        ) external;
    }

    #[derive(Debug)]
    contract Oracle {
        event OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved);
    }
}
