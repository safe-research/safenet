import { getAbiItem, parseAbi, parseAbiItem, toEventSelector } from "viem";

export const COORDINATOR_KEY_GEN_EVENTS = parseAbi([
	"event KeyGen(bytes32 indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 indexed context)",
	"event KeyGenCommitted(bytes32 indexed gid, address participant, ((uint256 x, uint256 y) q, (uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment, bool committed)",
	"event KeyGenSecretShared(bytes32 indexed gid, address participant, ((uint256 x, uint256 y) y, uint256[] f) share, bool shared)",
	"event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed)",
	"event KeyGenComplained(bytes32 indexed gid, address plaintiff, address accused, bool compromised)",
	"event KeyGenComplaintResponded(bytes32 indexed gid, address plaintiff, address accused, uint256 secretShare)",
]);

export const COORDINATOR_SIGNING_INITIATED_EVENT = parseAbiItem(
	"event Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)",
);

export const COORDINATOR_SIGNING_PROGRESS_EVENTS = parseAbi([
	"event SignCompleted(bytes32 indexed sid, bytes32 indexed selectionRoot, ((uint256 x, uint256 y) r, uint256 z) signature)",
	"event SignRevealedNonces(bytes32 indexed sid, address participant, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces)",
	"event SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, address participant, uint256 z)",
]);

export const COORDINATOR_OTHER_EVENTS = parseAbi([
	"event Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment)",
]);

export const COORDINATOR_EVENTS = [
	COORDINATOR_SIGNING_INITIATED_EVENT,
	...COORDINATOR_SIGNING_PROGRESS_EVENTS,
	...COORDINATOR_KEY_GEN_EVENTS,
	...COORDINATOR_OTHER_EVENTS,
] as const;

export const COORDINATOR_KEY_GEN_SELECTORS = [
	"KeyGen" as const,
	"KeyGenCommitted" as const,
	"KeyGenSecretShared" as const,
	"KeyGenConfirmed" as const,
	"KeyGenComplained" as const,
	"KeyGenComplaintResponded" as const,
].map((eventName) =>
	toEventSelector(
		getAbiItem({
			abi: COORDINATOR_KEY_GEN_EVENTS,
			name: eventName,
		}),
	),
);

export const COORDINATOR_SIGNING_PROGRESS_SELECTORS = [
	"SignCompleted" as const,
	"SignRevealedNonces" as const,
	"SignShared" as const,
].map((eventName) =>
	toEventSelector(
		getAbiItem({
			abi: COORDINATOR_SIGNING_PROGRESS_EVENTS,
			name: eventName,
		}),
	),
);
