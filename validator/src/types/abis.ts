import { parseAbi } from "viem";

// TODO: placeholder
export const CONSENSUS_CORE_EVENTS = parseAbi([
	"event Transfer(address indexed from, address indexed to, uint256 value)",
	"event Approve(address indexed from, address indexed to, uint256 amount)",
]);

export const COORDINATOR_EVENTS = parseAbi([
	"event KeyGen(bytes32 indexed gid, bytes32 participants, uint64 count, uint64 threshold)",
	"event KeyGenAborted(bytes32 indexed gid)",
	"event KeyGenCommitted(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment)",
	"event KeyGenSecretShared(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y) y, uint256[] f) share)",
	"event Preprocess(bytes32 indexed gid, uint256 identifier, uint32 chunk, bytes32 commitment)",
	"event Sign(bytes32 indexed gid, bytes32 sid, bytes32 message, uint256 sequence)",
	"event SignRevealedNonces(bytes32 indexed sid, uint256 identifier, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces)",
    "event SignShare(bytes32 indexed sid, uint256 identifier, uint256 z)"
]);
