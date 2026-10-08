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

// Mirrors `getRequest`'s real single-struct return (`{ terms, progress }`, per
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
	latestBlock?: bigint;
}): PublicClient =>
	({
		getBlockNumber: vi.fn().mockResolvedValue(request.latestBlock ?? 0n),
		readContract: vi.fn().mockResolvedValue({
			terms: {
				commitDeadline: request.commitDeadline ?? 0n,
				daoFeeShare: 0,
				revealDeadline: request.revealDeadline ?? 0n,
				bondTarget: request.bondTarget ?? 0n,
				sponsor: SPONSOR,
				slashAmount: 0n,
			},
			progress: {
				state: request.state,
				fee: request.fee ?? 0n,
				arbitrationDeadline: 0n,
				committedCount: request.committedCount ?? 0,
				revealedCount: request.revealedCount ?? 0,
				approveSentinelCount: request.approveSentinelCount,
				denySentinelCount: request.denySentinelCount,
			},
		}),
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

		expect(result).toEqual({ kind: "sentinel", state: name, approveCount: 3n, denyCount: 1n, noVotes: false });
	});

	type SentinelRequest = Parameters<typeof makeSentinelProvider>[0];
	const BASE_REQUEST = { commitDeadline: 100n, revealDeadline: 103n, approveSentinelCount: 0, denySentinelCount: 0 };

	it.each<[string, SentinelRequest, string]>([
		[
			"a pending request with no commit past the commit deadline",
			{ ...BASE_REQUEST, state: 1, latestBlock: 101n },
			"PENDING",
		],
		[
			"a pending request whose commits were not revealed by the reveal deadline",
			{ ...BASE_REQUEST, state: 1, latestBlock: 104n, committedCount: 2 },
			"PENDING",
		],
		["a request that timed out without votes", { ...BASE_REQUEST, state: 5, latestBlock: 104n }, "TIMED_OUT"],
	])("flags %s as having no votes", async (_, request, state) => {
		const provider = makeSentinelProvider(request);

		const result = await loadVotingStatus({ provider, ...commonParams });

		expect(result).toMatchObject({ kind: "sentinel", state, noVotes: true });
	});

	it.each<[string, SentinelRequest]>([
		["a pending request with no commit at the commit deadline", { ...BASE_REQUEST, state: 1, latestBlock: 100n }],
		[
			"a pending request with unrevealed commits as its reveal window opens",
			{ ...BASE_REQUEST, state: 1, latestBlock: 101n, committedCount: 2 },
		],
		[
			"a pending request with unrevealed commits at its reveal deadline",
			{ ...BASE_REQUEST, state: 1, latestBlock: 103n, committedCount: 2 },
		],
		[
			"a pending request with a revealed vote past the reveal deadline",
			{ ...BASE_REQUEST, state: 1, latestBlock: 104n, committedCount: 2, revealedCount: 1, approveSentinelCount: 1 },
		],
		[
			"a request whose dispute ended without a ruling",
			{
				...BASE_REQUEST,
				state: 5,
				latestBlock: 104n,
				committedCount: 2,
				revealedCount: 2,
				approveSentinelCount: 1,
				denySentinelCount: 1,
			},
		],
	])("does not flag %s as having no votes", async (_, request) => {
		const provider = makeSentinelProvider(request);

		const result = await loadVotingStatus({ provider, ...commonParams });

		expect(result).toMatchObject({ kind: "sentinel", noVotes: false });
	});

	it("does not flag a vote revealed while the status was loading", async () => {
		// Each answer comes from the current head, which moves two blocks per answer, and the only
		// committed sentinel reveals at block 103. If the block is read after the request, or in
		// parallel with it (the head answers last), it is past the deadline while the counts predate
		// the reveal, and the vote would be flagged.
		let head = 102n;
		const answer = async <T>(read: (block: bigint) => T, slow: boolean) => {
			if (slow) await new Promise((resolve) => setTimeout(resolve));
			const block = head;
			head += 2n;
			return read(block);
		};
		const request = (revealed: number) =>
			makeSentinelProvider({
				...BASE_REQUEST,
				state: 1,
				committedCount: 1,
				revealedCount: revealed,
				approveSentinelCount: revealed,
			}).readContract({} as never);
		const provider = {
			getChainId: async () => CHAIN_ID,
			getBlockNumber: () => answer((block) => block, true),
			readContract: () => answer((block) => request(block < 103n ? 0 : 1), false),
		} as unknown as PublicClient;

		const result = await loadVotingStatus({ provider, ...commonParams });

		expect(result).toMatchObject({ kind: "sentinel", noVotes: false });
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
