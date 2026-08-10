import { getAbiItem, parseAbi, toEventSelector } from "viem";

// Read-only surface of `SentinelOracle` (commit/reveal).
export const sentinelOracleAbi = parseAbi([
	"function getRequest(bytes32 requestId) view returns ((address proposer, uint256 fee, uint256 bondTarget, uint256 commitDeadline, uint256 revealDeadline, uint8 state, uint256 committedCount, uint256 revealedCount, uint256 approveSentinelCount, uint256 denySentinelCount))",
	"event Committed(bytes32 indexed requestId, address indexed sentinel, uint256 bondAmount)",
	"event Revealed(bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, string reason)",
]);

// `Committed`/`Revealed` share the same indexed-topic layout (`requestId`, `sentinel`), so both
// can be fetched with a single `eth_getLogs` call filtered by `requestId`.
export const sentinelVoteEventSelectors = ["Committed" as const, "Revealed" as const].map((eventName) =>
	toEventSelector(getAbiItem({ abi: sentinelOracleAbi, name: eventName })),
);

// `IOracle.OracleResult`, common to every oracle implementation (including non-Sentinel ones
// like `AlwaysApproveOracle`) — the generic fallback when `getRequest` isn't `SentinelOracle`-shaped.
export const oracleAbi = parseAbi([
	"event OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved)",
]);
