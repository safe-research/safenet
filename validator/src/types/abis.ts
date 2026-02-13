import { parseAbi, parseAbiItem } from "viem";

export const CONSENSUS_EPOCH_STAGED_EVENT = parseAbiItem(
	"event EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, (uint256 x, uint256 y) groupKey, ((uint256 x, uint256 y) r, uint256 z) attestation)",
);

export const CONSENSUS_TRANSACTION_PROPOSED_EVENT = parseAbiItem(
	"event TransactionProposed(bytes32 indexed transactionHash, uint256 indexed chainId, address indexed safe, uint64 epoch, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction)",
);

export const CONSENSUS_OTHER_EVENTS = parseAbi([
	"event EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, (uint256 x, uint256 y) groupKey)",
	"event TransactionAttested(bytes32 indexed transactionHash, uint64 epoch, ((uint256 x, uint256 y) r, uint256 z) attestation)",
]);

export const CONSENSUS_EVENTS = [
	CONSENSUS_EPOCH_STAGED_EVENT,
	CONSENSUS_TRANSACTION_PROPOSED_EVENT,
	...CONSENSUS_OTHER_EVENTS,
] as const;

export const COORDINATOR_SIGN_EVENT = parseAbiItem(
	"event Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)",
);

export const COORDINATOR_SIGN_COMPLETED_EVENT = parseAbiItem(
	"event SignCompleted(bytes32 indexed sid, bytes32 indexed selectionRoot, ((uint256 x, uint256 y) r, uint256 z) signature)",
);

export const COORDINATOR_OTHER_EVENTS = parseAbi([
	"event KeyGen(bytes32 indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 context)",
	"event KeyGenCommitted(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment, bool committed)",
	"event KeyGenSecretShared(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y) y, uint256[] f) share, bool shared)",
	"event KeyGenConfirmed(bytes32 indexed gid, uint256 identifier, bool confirmed)",
	"event KeyGenComplained(bytes32 indexed gid, uint256 plaintiff, uint256 accused, bool compromised)",
	"event KeyGenComplaintResponded(bytes32 indexed gid, uint256 plaintiff, uint256 accused, uint256 secretShare)",
	"event Preprocess(bytes32 indexed gid, uint256 identifier, uint64 chunk, bytes32 commitment)",
	"event SignRevealedNonces(bytes32 indexed sid, uint256 identifier, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces)",
	"event SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, uint256 identifier, uint256 z)",
]);

export const COORDINATOR_EVENTS = [
	COORDINATOR_SIGN_EVENT,
	COORDINATOR_SIGN_COMPLETED_EVENT,
	...COORDINATOR_OTHER_EVENTS,
] as const;

export const ALL_EVENTS = [...CONSENSUS_EVENTS, ...COORDINATOR_EVENTS] as const;

export const COORDINATOR_FUNCTIONS = parseAbi([
	"error AlreadyRegistered()",
	"error InvalidKeyGenCommitment()",
	"error NotParticipating()",
	"error GroupNotInitialized()",
	"error GroupNotCommitted()",
	"error InvalidMessage()",
	"function keyGenAndCommit(bytes32 participants, uint16 count, uint16 threshold, bytes32 context, uint256 identifier, bytes32[] poap, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment) external",
	"function keyGenCommit(bytes32 gid, uint256 identifier, bytes32[] poap, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment) external",
	"function keyGenSecretShare(bytes32 gid, ((uint256 x, uint256 y) y, uint256[] f) share) external",
	"function keyGenComplain(bytes32 gid, uint256 accused) external",
	"function keyGenComplaintResponse(bytes32 gid, uint256 plaintiff, uint256 secretShare) external",
	"function keyGenConfirm(bytes32 gid) external",
	"function keyGenConfirmWithCallback(bytes32 gid, (address target, bytes context) callback) external",
	"function preprocess(bytes32 id, bytes32 commitment) external returns (uint32 chunk)",
	"function sign(bytes32 gid, bytes32 message)",
	"function signRevealNonces(bytes32 sid, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces, bytes32[] proof) external",
	"function signShare(bytes32 sid, ((uint256 x, uint256 y) r, bytes32 root) selection, ((uint256 x, uint256 y) r, uint256 z, uint256 l) share, bytes32[] proof) external",
	"function signShareWithCallback(bytes32 sid, ((uint256 x, uint256 y) r, bytes32 root) selection, ((uint256 x, uint256 y) r, uint256 z, uint256 l) share, bytes32[] proof, (address target, bytes context) callback) external",
]);

export const CONSENSUS_FUNCTIONS = parseAbi([
	"error InvalidRollover()",
	"error GroupNotInitialized()",
	"error GroupNotCommitted()",
	"error InvalidMessage()",
	"error NotSigned()",
	"error WrongSignature()",
	"function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 group) external",
	"function stageEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 group, bytes32 signature) external",
	"function attestTransaction(uint64 epoch, bytes32 transactionHash, bytes32 signature) external",
]);
