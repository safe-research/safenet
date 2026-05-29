import type { Address, PublicClient } from "viem";
import { zeroAddress } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// loadCoordinator is module-private. We test it indirectly via
// loadLatestAttestationStatus: the coordinator address resolved by
// loadCoordinator is then passed to provider.getLogs as the `address` filter,
// which lets us assert which address was resolved.
//
// Each test resets the module so the module-level address cache is cleared.

const CONSENSUS: Address = "0x1111111111111111111111111111111111111111";
const COORDINATOR_FROM_GETTER: Address = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

// Valid 32-byte hex (required by safeTxProposalHash's hashTypedData)
const SAFE_TX_HASH: `0x${string}` = `0x${"ab".repeat(32)}`;

type LoadLatestAttestationStatus = typeof import("./signing").loadLatestAttestationStatus;

const loadModule = async () => {
	vi.resetModules();
	const mod = await import("./signing");
	return mod.loadLatestAttestationStatus as LoadLatestAttestationStatus;
};

// Builds a minimal mock PublicClient. getLogs returns [] so that
// loadLatestAttestationStatus returns null without further processing after
// the coordinator address is resolved.
const makeProvider = ({
	readContractImpl,
	getLogsImpl,
	requestImpl,
}: {
	readContractImpl?: (args: { functionName: string }) => unknown;
	getLogsImpl?: (args: unknown) => unknown;
	requestImpl?: (args: unknown) => unknown;
} = {}): PublicClient =>
	({
		getBlockNumber: vi.fn().mockResolvedValue(10000n),
		getChainId: vi.fn().mockResolvedValue(1),
		getLogs: getLogsImpl ? vi.fn(getLogsImpl) : vi.fn().mockResolvedValue([]),
		readContract: readContractImpl ? vi.fn(readContractImpl) : vi.fn(),
		request: requestImpl ? vi.fn(requestImpl) : vi.fn().mockResolvedValue([]),
	}) as unknown as PublicClient;

const baseArgs = {
	consensus: CONSENSUS,
	safeTxHash: SAFE_TX_HASH,
	epoch: 0n,
	proposedAt: 0n,
	maxBlockRange: 1000n,
};

describe("loadCoordinator (via loadLatestAttestationStatus)", () => {
	beforeEach(() => {
		vi.resetModules();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("uses getCoordinator() to fetch the coordinator address", async () => {
		const provider = makeProvider({
			readContractImpl: async () => COORDINATOR_FROM_GETTER,
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });

		expect(provider.getLogs).toHaveBeenCalledWith(expect.objectContaining({ address: COORDINATOR_FROM_GETTER }));
	});

	it("throws when getCoordinator() fails", async () => {
		const provider = makeProvider({
			readContractImpl: async () => {
				throw new Error("not found");
			},
		});

		const load = await loadModule();
		await expect(load({ provider, ...baseArgs })).rejects.toThrow("not found");
	});

	it("caches the coordinator and does not call the provider again for the same consensus", async () => {
		const provider = makeProvider({
			readContractImpl: async () => COORDINATOR_FROM_GETTER,
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });
		await load({ provider, ...baseArgs });

		// readContract should only be called once despite two loadLatestAttestationStatus invocations
		expect(provider.readContract).toHaveBeenCalledTimes(1);
	});

	it("does not cache failures — retries on next call", async () => {
		let callCount = 0;
		const provider = makeProvider({
			readContractImpl: async () => {
				callCount++;
				if (callCount === 1) {
					throw new Error("not found");
				}
				// Second attempt succeeds
				return COORDINATOR_FROM_GETTER;
			},
		});

		const load = await loadModule();
		// First call fails
		await expect(load({ provider, ...baseArgs })).rejects.toThrow();
		// Second call succeeds because the failure was not cached
		await load({ provider, ...baseArgs });

		expect(provider.getLogs).toHaveBeenCalledWith(expect.objectContaining({ address: COORDINATOR_FROM_GETTER }));
	});
});

// Selectors for coordinator signing progress events
const SIGN_SELECTOR = "0xb48d242879f9f3df555c800db966f65cba128c7213198748fa202ed54e092691";
const SIGN_DECLINED_SELECTOR = "0xe6d872ab6c2f6512d506498a50ba8ba1bbc2bb3c19439ad7c6ff3d74465277d7";
const SIGN_REVEALED_NONCES_SELECTOR = "0xa8415ae8824ba92b55156b0447b9b9bbc3ba63988b076fb0c8d8e180893d1a46";
const SIGN_SHARED_SELECTOR = "0x25a4d6e8d11a9fdc20ffdd826473485ae4cdd453271726c072a16836c1882e7c";

const SID: `0x${string}` = `0x${"aa".repeat(32)}`;
const SELECTION_ROOT: `0x${string}` = `0x${"bb".repeat(32)}`;
const PARTICIPANT_A: Address = "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa";
const PARTICIPANT_B: Address = "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB";
// Address padded to 32 bytes (for use as topic)
const padAddress = (addr: string) => `0x${"00".repeat(12)}${addr.slice(2).toLowerCase()}`;

// Base raw log — shared fields for all coordinator progress events
const makeLog = (topics: string[], data: string, blockNumber = "0x0") => ({
	address: COORDINATOR_FROM_GETTER,
	blockHash: `0x${"cc".repeat(32)}`,
	blockNumber,
	data,
	logIndex: "0x0",
	removed: false,
	topics,
	transactionHash: `0x${"dd".repeat(32)}`,
	transactionIndex: "0x0",
});

// Builds a Sign-initiated raw log
const makeSignLog = (_sid: string) => ({
	...makeLog(
		[SIGN_SELECTOR, padAddress(zeroAddress), `0x${"00".repeat(32)}`, SAFE_TX_HASH],
		`0x${"00".repeat(40)}`, // sid (bytes32) + sequence (uint64)
		"0x1",
	),
});

// Builds a SignDeclined raw log
const makeDeclinedLog = (sid: string, participant: Address, blockNumber = "0x2") => ({
	...makeLog([SIGN_DECLINED_SELECTOR, sid, padAddress(participant)], "0x", blockNumber),
	logIndex: "0x1",
});

// Builds a SignRevealedNonces raw log
const makeRevealedNoncesLog = (sid: string, participant: Address, blockNumber = "0x2") => ({
	...makeLog(
		[SIGN_REVEALED_NONCES_SELECTOR, sid],
		// participant (address, 32 bytes) + nonces struct (4 × uint256 = 128 bytes)
		`0x${"00".repeat(12)}${participant.slice(2).toLowerCase()}${"00".repeat(128)}`,
		blockNumber,
	),
	logIndex: "0x4",
});

// Builds a SignShared raw log
// participant is non-indexed and encoded as the first 32 bytes of data alongside z (uint256)
const makeSharedLog = (sid: string, _participant: Address, blockNumber = "0x3") => ({
	...makeLog([SIGN_SHARED_SELECTOR, sid, SELECTION_ROOT], `0x${"00".repeat(32)}`, blockNumber),
	logIndex: "0x2",
});

// Full mock provider for declined-behaviour tests (Sign event present).
// The production code calls provider.request({ method: "eth_getLogs" }) directly for
// fine-grained topic filtering, so we mock request to return the progress logs.
const makeFullProvider = ({ progressLogs = [] }: { progressLogs?: unknown[] }): PublicClient => {
	const signLog = makeSignLog(SID);
	return makeProvider({
		readContractImpl: async () => COORDINATOR_FROM_GETTER,
		getLogsImpl: async () => [
			{
				...signLog,
				blockNumber: 1n,
				args: {
					initiator: zeroAddress,
					gid: `0x${"00".repeat(32)}`,
					message: SAFE_TX_HASH,
					sid: SID,
					sequence: 0n,
				},
				eventName: "Sign",
				logIndex: 0,
			},
		],
		requestImpl: async () => progressLogs,
	});
};

describe("loadLatestAttestationStatus — declined field", () => {
	beforeEach(() => {
		vi.resetModules();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("populates declined when a SignDeclined event is present", async () => {
		const declinedLog = makeDeclinedLog(SID, PARTICIPANT_A);
		const provider = makeFullProvider({ progressLogs: [declinedLog] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toEqual([{ address: PARTICIPANT_A, block: 2n }]);
		expect(result?.signed).toEqual([]);
	});

	it("excludes a participant from declined when they also have a SignShared event", async () => {
		// Participant A declines first, then signs
		const declinedLog = makeDeclinedLog(SID, PARTICIPANT_A, "0x2");
		// SignShared has participant encoded as non-indexed data: address (32 bytes) + z (32 bytes)
		const sharedData = `0x${"00".repeat(12)}${PARTICIPANT_A.slice(2).toLowerCase()}${"00".repeat(32)}`;
		const sharedLog = {
			...makeSharedLog(SID, PARTICIPANT_A, "0x3"),
			data: sharedData,
		};
		const provider = makeFullProvider({ progressLogs: [declinedLog, sharedLog] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toEqual([]);
		// Participant A appears in signed
		expect(result?.signed.map((s) => s.address)).toContain(PARTICIPANT_A);
	});

	it("deduplicates repeated SignDeclined events from the same participant", async () => {
		const log1 = makeDeclinedLog(SID, PARTICIPANT_A, "0x2");
		const log2 = { ...makeDeclinedLog(SID, PARTICIPANT_A, "0x3"), logIndex: "0x3" };
		const provider = makeFullProvider({ progressLogs: [log1, log2] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toHaveLength(1);
		expect(result?.declined[0].address).toBe(PARTICIPANT_A);
	});

	it("keeps participants from different addresses in declined independently", async () => {
		const logA = makeDeclinedLog(SID, PARTICIPANT_A, "0x2");
		const logB = { ...makeDeclinedLog(SID, PARTICIPANT_B, "0x3"), logIndex: "0x3" };
		const provider = makeFullProvider({ progressLogs: [logA, logB] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toHaveLength(2);
		expect(result?.declined.map((d) => d.address)).toContain(PARTICIPANT_A);
		expect(result?.declined.map((d) => d.address)).toContain(PARTICIPANT_B);
	});

	it("excludes a participant from declined when they also have a SignRevealedNonces event", async () => {
		const committedLog = makeRevealedNoncesLog(SID, PARTICIPANT_A, "0x2");
		const declinedLog = { ...makeDeclinedLog(SID, PARTICIPANT_A, "0x3"), logIndex: "0x3" };
		const provider = makeFullProvider({ progressLogs: [committedLog, declinedLog] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toEqual([]);
		expect(result?.committed.map((c) => c.address)).toContain(PARTICIPANT_A);
	});
});
