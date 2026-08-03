import type { Address, Hex, PublicClient } from "viem";
import { zeroAddress } from "viem";
import { describe, expect, it, vi } from "vitest";
import { loadVotingStatus } from "./votingStatus";

const ORACLE: Address = "0x1234567890123456789012345678901234567890";
const PROPOSER: Address = "0x9999999999999999999999999999999999999999";
const REQUEST_ID: Hex = `0x${"ab".repeat(32)}`;

const makeSentinelProvider = (request: {
	proposer?: Address;
	fee?: bigint;
	bondTarget?: bigint;
	commitDeadline?: bigint;
	revealDeadline?: bigint;
	state: number;
	committedCount?: bigint;
	revealedCount?: bigint;
	approveSentinelCount: bigint;
	denySentinelCount: bigint;
}): PublicClient =>
	({
		readContract: vi.fn().mockResolvedValue({
			proposer: PROPOSER,
			fee: 0n,
			bondTarget: 0n,
			commitDeadline: 0n,
			revealDeadline: 0n,
			committedCount: 0n,
			revealedCount: 0n,
			...request,
		}),
	}) as unknown as PublicClient;

const makeGenericProvider = (logs: unknown[] = []): PublicClient =>
	({
		readContract: vi.fn().mockRejectedValue(new Error("function selector not recognized")),
		getBlockNumber: vi.fn().mockResolvedValue(1000n),
		getLogs: vi.fn().mockResolvedValue(logs),
	}) as unknown as PublicClient;

const makeOracleResultLog = (approved: boolean, blockNumber = 1n, logIndex = 0) => ({
	args: { requestId: REQUEST_ID, proposer: ORACLE, result: "0x" as Hex, approved },
	blockNumber,
	logIndex,
});

describe("loadVotingStatus", () => {
	it.each([
		[0, "PENDING"],
		[1, "FROZEN"],
		[2, "RESOLVED_APPROVED"],
		[3, "RESOLVED_DENIED"],
		[4, "TIMED_OUT"],
	] as const)("maps SentinelOracleRequest.State ordinal %i to %s", async (state, name) => {
		const provider = makeSentinelProvider({ state, approveSentinelCount: 3n, denySentinelCount: 1n });

		const result = await loadVotingStatus({ provider, oracle: ORACLE, requestId: REQUEST_ID, maxBlockRange: 500n });

		expect(result).toEqual({ kind: "sentinel", state: name, approveCount: 3n, denyCount: 1n });
	});

	it("returns null when getRequest resolves a never-posted requestId (zero-initialized struct)", async () => {
		const provider = makeSentinelProvider({
			proposer: zeroAddress,
			state: 0,
			approveSentinelCount: 0n,
			denySentinelCount: 0n,
		});

		const result = await loadVotingStatus({ provider, oracle: ORACLE, requestId: REQUEST_ID, maxBlockRange: 500n });

		expect(result).toBeNull();
	});

	it("falls back to OracleResult logs when getRequest reverts", async () => {
		const provider = makeGenericProvider([makeOracleResultLog(true)]);

		const result = await loadVotingStatus({ provider, oracle: ORACLE, requestId: REQUEST_ID, maxBlockRange: 500n });

		expect(result).toEqual({ kind: "generic", approved: true });
	});

	it("returns null when getRequest reverts and no OracleResult log exists yet", async () => {
		const provider = makeGenericProvider([]);

		const result = await loadVotingStatus({ provider, oracle: ORACLE, requestId: REQUEST_ID, maxBlockRange: 500n });

		expect(result).toBeNull();
	});
});
