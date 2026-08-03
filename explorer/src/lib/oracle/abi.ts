import { parseAbi } from "viem";

// Read-only surface of `SentinelOracleV2` (commit/reveal). V1 (`SentinelOracle`) is deprecated
// and out of scope — there is no dual-version handling here.
export const sentinelOracleV2Abi = parseAbi([
	"function getRequest(bytes32 requestId) view returns ((address proposer, uint256 fee, uint256 bondTarget, uint256 commitDeadline, uint256 revealDeadline, uint8 state, uint256 committedCount, uint256 revealedCount, uint256 approveSentinelCount, uint256 denySentinelCount))",
	"event Committed(bytes32 indexed requestId, address indexed sentinel, uint256 bondAmount)",
	"event Revealed(bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, string reason)",
]);

// `IOracle.OracleResult`, common to every oracle implementation (including non-Sentinel ones
// like `AlwaysApproveOracle`) — the generic fallback when `getRequest` isn't `SentinelOracleV2`-shaped.
export const oracleAbi = parseAbi([
	"event OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved)",
]);
