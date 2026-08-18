import { getAbiItem, parseAbi, toEventSelector } from "viem";

// Read-only surface of `SentinelOracle` (commit/reveal). `getRequest` returns a single
// `SentinelOracleRequest.T` struct, field-for-field with `Terms` (write-once creation params) and
// `Progress` (everything the request lifecycle mutates) — see
// contracts/src/libraries/SentinelOracleRequests.sol, including that struct's `_padding` fields
// (real ABI members, not decoration — omitting them misaligns every field decoded after them).
// viem decodes this single-output function straight to that struct, so callers read
// `result.terms`/`result.progress`, not `result[0]`/`result[1]`.
export const sentinelOracleAbi = parseAbi([
	"struct Terms { uint64 commitDeadline; uint24 daoFeeShare; uint64 revealDeadline; uint96 bondTarget; uint8 _padding; address sponsor; uint96 slashAmount; }",
	"struct Progress { uint8 state; uint96 fee; uint64 arbitrationDeadline; uint16 committedCount; uint16 revealedCount; uint16 approveSentinelCount; uint16 denySentinelCount; uint24 _padding; }",
	"struct Request { Terms terms; Progress progress; }",
	"function getRequest(bytes32 requestId) view returns (Request)",
	"event Committed(bytes32 indexed requestId, address indexed sentinel, uint96 bondAmount)",
	"event Revealed(bytes32 indexed requestId, address indexed sentinel, bool approved, uint96 bondAmount, string reason)",
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
