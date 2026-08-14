import { getAbiItem, parseAbi, toEventSelector } from "viem";

// Read-only surface of `SentinelOracle` (commit/reveal). `getRequest` returns two structs,
// field-for-field with `SentinelOracleRequest.Terms` (write-once creation params) and
// `SentinelOracleRequest.Progress` (everything the request lifecycle mutates) — see
// contracts/src/libraries/SentinelOracleRequests.sol. viem decodes multi-value returns as a
// positional tuple `[terms, progress]` regardless of the (cosmetic, docs-only) output names below.
export const sentinelOracleAbi = parseAbi([
	"function getRequest(bytes32 requestId) view returns ((uint64 commitDeadline, uint24 daoFeeShare, uint64 revealDeadline, uint96 bondTarget, address sponsor, uint96 slashAmount) terms, (uint8 state, uint96 fee, uint64 arbitrationDeadline, uint16 committedCount, uint16 revealedCount, uint16 approveSentinelCount, uint16 denySentinelCount) progress)",
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
