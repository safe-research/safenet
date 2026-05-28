import type { Address, PublicClient } from "viem";
import { zeroAddress } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// loadCoordinator is module-private. We test it indirectly via
// loadLatestAttestationStatus: the coordinator address resolved by
// loadCoordinator is then passed to provider.getLogs as the `address` filter,
// which lets us assert which address was resolved.
//
// Each test resets the module so the module-level address cache is cleared.

const CONSENSUS = "0x1111111111111111111111111111111111111111" as Address;
const COORDINATOR_FROM_GETTER = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" as Address;

// Valid 32-byte hex (required by safeTxProposalHash's hashTypedData)
const SAFE_TX_HASH = `0x${"ab".repeat(32)}` as `0x${string}`;

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
}: {
	readContractImpl?: (args: { functionName: string }) => unknown;
}): PublicClient =>
	({
		getBlockNumber: vi.fn().mockResolvedValue(10000n),
		getChainId: vi.fn().mockResolvedValue(1),
		getLogs: vi.fn().mockResolvedValue([]),
		readContract: readContractImpl ? vi.fn(readContractImpl) : vi.fn(),
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
const SIGN_SHARED_SELECTOR = "0x25a4d6e8d11a9fdc20ffdd826473485ae4cdd453271726c072a16836c1882e7c";

const SID = `0x${"aa".repeat(32)}` as `0x${string}`;
const SID_PADDED = SID; // bytes32 is already padded
const SELECTION_ROOT = `0x${"bb".repeat(32)}` as `0x${string}`;
const PARTICIPANT_A = "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa" as Address;
const PARTICIPANT_B = "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB" as Address;
// Address padded to 32 bytes (for use as topic)
const padAddress = (addr: string) => `0x${"00".repeat(12)}${addr.slice(2).toLowerCase()}`;

// Builds a Sign-initiated raw log
const makeSignLog = (_sid: string) => ({
	address: COORDINATOR_FROM_GETTER,
	blockHash: `0x${"cc".repeat(32)}`,
	blockNumber: "0x1",
	data: `0x${"00".repeat(32)}${"00".repeat(8)}`, // sid (bytes32) + sequence (uint64)
	logIndex: "0x0",
	removed: false,
	topics: [
		SIGN_SELECTOR,
		padAddress(zeroAddress), // initiator
		`0x${"00".repeat(32)}`, // gid
		SAFE_TX_HASH, // message
	],
	transactionHash: `0x${"dd".repeat(32)}`,
	transactionIndex: "0x0",
});

// Builds a SignDeclined raw log
const makeDeclinedLog = (sid: string, participant: Address, blockNumber = "0x2") => ({
	address: COORDINATOR_FROM_GETTER,
	blockHash: `0x${"cc".repeat(32)}`,
	blockNumber,
	data: "0x",
	logIndex: "0x1",
	removed: false,
	topics: [SIGN_DECLINED_SELECTOR, sid, padAddress(participant)],
	transactionHash: `0x${"dd".repeat(32)}`,
	transactionIndex: "0x0",
});

// Builds a SignShared raw log
const makeSharedLog = (sid: string, _participant: Address, blockNumber = "0x3") => ({
	address: COORDINATOR_FROM_GETTER,
	blockHash: `0x${"cc".repeat(32)}`,
	blockNumber,
	data: `0x${"00".repeat(32)}`, // z (uint256)
	logIndex: "0x2",
	removed: false,
	topics: [SIGN_SHARED_SELECTOR, sid, SELECTION_ROOT],
	// participant is non-indexed, encoded in data alongside z — but viem's parseEventLogs
	// reads from data, so we pack participant (padded to 32) + z (uint256 = 0)
	transactionHash: `0x${"dd".repeat(32)}`,
	transactionIndex: "0x0",
});

// Full mock provider for declined-behavior tests (Sign event present)
const makeFullProvider = ({ progressLogs = [] }: { progressLogs?: unknown[] }): PublicClient => {
	const signLog = makeSignLog(SID);
	return {
		getBlockNumber: vi.fn().mockResolvedValue(10000n),
		getChainId: vi.fn().mockResolvedValue(1),
		// getLogs returns the Sign-initiated event
		getLogs: vi.fn().mockResolvedValue([
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
		]),
		// provider.request handles eth_getLogs for progress events
		request: vi.fn().mockResolvedValue(progressLogs),
		readContract: vi.fn().mockResolvedValue(COORDINATOR_FROM_GETTER),
	} as unknown as PublicClient;
};

describe("loadLatestAttestationStatus — declined field", () => {
	beforeEach(() => {
		vi.resetModules();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("populates declined when a SignDeclined event is present", async () => {
		const declinedLog = makeDeclinedLog(SID_PADDED, PARTICIPANT_A);
		const provider = makeFullProvider({ progressLogs: [declinedLog] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toEqual([{ address: PARTICIPANT_A, block: 2n }]);
		expect(result?.signed).toEqual([]);
	});

	it("excludes a participant from declined when they also have a SignShared event", async () => {
		// Participant A declines first, then signs
		const declinedLog = makeDeclinedLog(SID_PADDED, PARTICIPANT_A, "0x2");
		// SignShared has participant encoded as non-indexed data: address (32 bytes) + z (32 bytes)
		const sharedData = `0x${"00".repeat(12)}${PARTICIPANT_A.slice(2).toLowerCase()}${"00".repeat(32)}`;
		const sharedLog = {
			...makeSharedLog(SID_PADDED, PARTICIPANT_A, "0x3"),
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
		const log1 = makeDeclinedLog(SID_PADDED, PARTICIPANT_A, "0x2");
		const log2 = { ...makeDeclinedLog(SID_PADDED, PARTICIPANT_A, "0x3"), logIndex: "0x3" };
		const provider = makeFullProvider({ progressLogs: [log1, log2] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toHaveLength(1);
		expect(result?.declined[0].address).toBe(PARTICIPANT_A);
	});

	it("keeps participants from different addresses in declined independently", async () => {
		const logA = makeDeclinedLog(SID_PADDED, PARTICIPANT_A, "0x2");
		const logB = { ...makeDeclinedLog(SID_PADDED, PARTICIPANT_B, "0x3"), logIndex: "0x3" };
		const provider = makeFullProvider({ progressLogs: [logA, logB] });
		const load = await loadModule();

		const result = await load({ provider, ...baseArgs });

		expect(result).not.toBeNull();
		expect(result?.declined).toHaveLength(2);
		expect(result?.declined.map((d) => d.address)).toContain(PARTICIPANT_A);
		expect(result?.declined.map((d) => d.address)).toContain(PARTICIPANT_B);
	});
});
