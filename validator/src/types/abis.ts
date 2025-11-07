import { parseAbi } from "viem";

// TODO: placeholder
export const CONSENSUS_CORE_EVENTS = parseAbi([
	"event Transfer(address indexed from, address indexed to, uint256 value)",
	"event Approve(address indexed from, address indexed to, uint256 amount)",
]);

export const COORDINATOR_EVENTS = parseAbi([
	"event KeyGen(bytes32 indexed id, bytes32 participants, uint128 count, uint128 threshold)",
	"event KeyGenAborted(bytes32 indexed id)",
	"event KeyGenCommitted(bytes32 indexed id, uint256 index, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment)",
	"event KeyGenSecretShared(bytes32 indexed id, uint256 index, ((uint256 x, uint256 y) y, uint256[] f) share)",
]);
