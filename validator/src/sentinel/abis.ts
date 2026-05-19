import { parseAbi, parseAbiItem } from "viem";

export const SENTINEL_NEW_REQUEST_EVENT = parseAbiItem(
	"event NewRequest(bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 bondTarget, uint256 deadline)",
);

export const SENTINEL_ORACLE_RESULT_EVENT = parseAbiItem(
	"event OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved)",
);

export const SENTINEL_DISPUTE_RESOLVED_EVENT = parseAbiItem(
	"event DisputeResolved(bytes32 indexed requestId, uint8 outcome, uint256 slashed)",
);

export const SENTINEL_CLAIMED_EVENT = parseAbiItem(
	"event Claimed(bytes32 indexed requestId, address indexed sentinel, uint256 bondReturn, uint256 feeReward)",
);

export const SENTINEL_COMMITTED_EVENT = parseAbiItem(
	"event Committed(bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, uint256 position)",
);

export const SENTINEL_EVENTS = [
	SENTINEL_NEW_REQUEST_EVENT,
	SENTINEL_ORACLE_RESULT_EVENT,
	SENTINEL_DISPUTE_RESOLVED_EVENT,
	SENTINEL_COMMITTED_EVENT,
] as const;

export const SENTINEL_ORACLE_FUNCTIONS = parseAbi([
	"function commitApprove(bytes32 requestId, uint256 bondAmount) external",
	"function commitDeny(bytes32 requestId, uint256 bondAmount) external",
	"function finalize(bytes32 requestId) external",
	"function claim(bytes32 requestId) external",
	"function FEE_TOKEN() external view returns (address)",
]);

export const ERC20_FUNCTIONS = parseAbi([
	"function approve(address spender, uint256 amount) external returns (bool)",
	"function allowance(address owner, address spender) external view returns (uint256)",
]);
