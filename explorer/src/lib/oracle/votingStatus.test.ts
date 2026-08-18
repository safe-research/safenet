import type { Address, Hex, PublicClient } from "viem";
import { describe, expect, it, vi } from "vitest";
import { loadVotingStatus } from "./votingStatus";

const ORACLE: Address = "0x1234567890123456789012345678901234567890";
const SPONSOR: Address = "0x9999999999999999999999999999999999999999";
const CONSENSUS: Address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
const EPOCH = 1n;
const SAFE_TX_HASH: Hex = "0xfe8b85e8d090b16fe8f142d3c9292dc1fc77daf9eb4af8f7cf4a7707d95f4028";
const REQUEST_ID: Hex = `0x${"ab".repeat(32)}`;
const CHAIN_ID = 1;

const commonParams = {
	oracle: ORACLE,
	consensus: CONSENSUS,
	epoch: EPOCH,
	safeTxHash: SAFE_TX_HASH,
	oracleData: "0x" as Hex,
	maxBlockRange: 500n,
};

// Mirrors `getRequest`'s real two-tuple return (`[Terms, Progress]`, per
// contracts/src/libraries/SentinelOracleRequests.sol) rather than the flat field list
// `loadVotingStatus` actually reads from, so this stays a faithful stand-in for what
// `readContract` decodes.
const makeSentinelProvider = (request: {
	fee?: bigint;
	bondTarget?: bigint;
	commitDeadline?: bigint;
	revealDeadline?: bigint;
	state: number;
	committedCount?: number;
	revealedCount?: number;
	approveSentinelCount: number;
	denySentinelCount: number;
}): PublicClient =>
	({
		readContract: vi.fn().mockResolvedValue([
			{
				commitDeadline: request.commitDeadline ?? 0n,
				daoFeeShare: 0,
				revealDeadline: request.revealDeadline ?? 0n,
				bondTarget: request.bondTarget ?? 0n,
				sponsor: SPONSOR,
				slashAmount: 0n,
			},
			{
				state: request.state,
				fee: request.fee ?? 0n,
				arbitrationDeadline: 0n,
				committedCount: request.committedCount ?? 0,
				revealedCount: request.revealedCount ?? 0,
				approveSentinelCount: request.approveSentinelCount,
				denySentinelCount: request.denySentinelCount,
			},
		]),
		getChainId: vi.fn().mockResolvedValue(CHAIN_ID),
	}) as unknown as PublicClient;

const makeGenericProvider = (logs: unknown[] = []): PublicClient =>
	({
		readContract: vi.fn().mockRejectedValue(new Error("function selector not recognized")),
		getBlockNumber: vi.fn().mockResolvedValue(1000n),
		getLogs: vi.fn().mockResolvedValue(logs),
		getChainId: vi.fn().mockResolvedValue(CHAIN_ID),
	}) as unknown as PublicClient;

const makeOracleResultLog = (approved: boolean, blockNumber = 1n, logIndex = 0) => ({
	args: { requestId: REQUEST_ID, proposer: ORACLE, result: "0x" as Hex, approved },
	blockNumber,
	logIndex,
});

describe("loadVotingStatus", () => {
	it.each([
		[1, "PENDING"],
		[2, "FROZEN"],
		[3, "RESOLVED_APPROVED"],
		[4, "RESOLVED_DENIED"],
		[5, "TIMED_OUT"],
	] as const)("maps SentinelOracleRequest.State ordinal %i to %s", async (state, name) => {
		const provider = makeSentinelProvider({ state, approveSentinelCount: 3, denySentinelCount: 1 });

		const result = await loadVotingStatus({ provider, ...commonParams });

		expect(result).toEqual({ kind: "sentinel", state: name, approveCount: 3n, denyCount: 1n });
	});

	it("returns null when getRequest resolves a never-posted requestId (zero-initialized struct, state NONE)", async () => {
		const provider = makeSentinelProvider({
			state: 0,
			approveSentinelCount: 0,
			denySentinelCount: 0,
		});

		const result = await loadVotingStatus({ provider, ...commonParams });

		expect(result).toBeNull();
	});

	it("falls back to OracleResult logs when getRequest reverts", async () => {
		const provider = makeGenericProvider([makeOracleResultLog(true)]);

		const result = await loadVotingStatus({ provider, ...commonParams });

		expect(result).toEqual({ kind: "generic", approved: true });
	});

	it("returns null when getRequest reverts and no OracleResult log exists yet", async () => {
		const provider = makeGenericProvider([]);

		const result = await loadVotingStatus({ provider, ...commonParams });

		expect(result).toBeNull();
	});
});
