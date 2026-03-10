import type { Address, PublicClient } from "viem";
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
