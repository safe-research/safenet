import { parseAbi } from "viem";

export const CONSENSUS_EVENTS = parseAbi([
	"event EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 timestamp, (uint256 x, uint256 y) groupKey)",
]);

export const COORDINATOR_EVENTS = parseAbi([
	"event KeyGen(bytes32 indexed gid, bytes32 participants, uint64 count, uint64 threshold, bytes32 context)",
	"event KeyGenAborted(bytes32 indexed gid)",
	"event KeyGenCommitted(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment)",
	"event KeyGenSecretShared(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y) y, uint256[] f) share)",
	"event Preprocess(bytes32 indexed gid, uint256 identifier, uint64 chunk, bytes32 commitment)",
	"event Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)",
	"event SignRevealedNonces(bytes32 indexed sid, uint256 identifier, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces)",
	"event SignShare(bytes32 indexed sid, uint256 identifier, uint256 z, bytes32 root)",
]);
