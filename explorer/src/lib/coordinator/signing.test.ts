import type { Address, PublicClient } from "viem";
import { ChainDoesNotSupportContract } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// loadCoordinator is module-private. We test it indirectly via
// loadLatestAttestationStatus: the coordinator address resolved by
// loadCoordinator is then passed to provider.getLogs as the `address` filter,
// which lets us assert which address was resolved.
//
// Each test resets the module so the module-level address cache is cleared.

const CONSENSUS = "0x1111111111111111111111111111111111111111" as Address;
const COORDINATOR_FROM_GETTER = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" as Address;
const COORDINATOR_FROM_UPPER = "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB" as Address;

// Valid 32-byte hex (required by safeTxProposalHash's hashTypedData)
const SAFE_TX_HASH = `0x${"ab".repeat(32)}` as `0x${string}`;

// Convenience factory for the error viem throws when Multicall3 is absent.
const noMulticall = () =>
	new ChainDoesNotSupportContract({
		chain: { id: 1, name: "test" } as ConstructorParameters<typeof ChainDoesNotSupportContract>[0]["chain"],
		contract: { name: "multicall3" },
	});

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
	multicallImpl,
	readContractImpl,
}: {
	multicallImpl?: (...args: unknown[]) => unknown;
	readContractImpl?: (args: { functionName: string }) => unknown;
}): PublicClient =>
	({
		getChainId: vi.fn().mockResolvedValue(1),
		getLogs: vi.fn().mockResolvedValue([]),
		multicall: multicallImpl ? vi.fn(multicallImpl) : vi.fn(),
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

	it("returns getCoordinator() result when both multicall entries succeed", async () => {
		const provider = makeProvider({
			multicallImpl: async () => [
				{ status: "success", result: COORDINATOR_FROM_UPPER },
				{ status: "success", result: COORDINATOR_FROM_GETTER },
			],
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });

		expect(provider.getLogs).toHaveBeenCalledWith(expect.objectContaining({ address: COORDINATOR_FROM_GETTER }));
	});

	it("returns COORDINATOR() result when only it succeeds in multicall", async () => {
		const provider = makeProvider({
			multicallImpl: async () => [
				{ status: "success", result: COORDINATOR_FROM_UPPER },
				{ status: "failure", error: new Error("not found") },
			],
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });

		expect(provider.getLogs).toHaveBeenCalledWith(expect.objectContaining({ address: COORDINATOR_FROM_UPPER }));
	});

	it("returns getCoordinator() result when only it succeeds in multicall", async () => {
		const provider = makeProvider({
			multicallImpl: async () => [
				{ status: "failure", error: new Error("not found") },
				{ status: "success", result: COORDINATOR_FROM_GETTER },
			],
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });

		expect(provider.getLogs).toHaveBeenCalledWith(expect.objectContaining({ address: COORDINATOR_FROM_GETTER }));
	});

	it("throws when both multicall entries fail", async () => {
		const provider = makeProvider({
			multicallImpl: async () => [
				{ status: "failure", error: new Error("not found") },
				{ status: "failure", error: new Error("not found") },
			],
		});

		const load = await loadModule();
		await expect(load({ provider, ...baseArgs })).rejects.toThrow(
			`Could not read coordinator from consensus contract ${CONSENSUS}`,
		);
	});

	it("re-throws unexpected multicall errors without falling back", async () => {
		const unexpectedError = new Error("RPC connection refused");
		const provider = makeProvider({
			multicallImpl: async () => {
				throw unexpectedError;
			},
		});

		const load = await loadModule();
		await expect(load({ provider, ...baseArgs })).rejects.toThrow(unexpectedError);
		// Individual readContract should never have been attempted
		expect(provider.readContract).not.toHaveBeenCalled();
	});

	it("falls back to individual readContract when Multicall3 is not deployed, preferring getCoordinator()", async () => {
		const provider = makeProvider({
			multicallImpl: async () => {
				throw noMulticall();
			},
			readContractImpl: async ({ functionName }: { functionName: string }) => {
				if (functionName === "getCoordinator") return COORDINATOR_FROM_GETTER;
				return COORDINATOR_FROM_UPPER;
			},
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });

		expect(provider.getLogs).toHaveBeenCalledWith(expect.objectContaining({ address: COORDINATOR_FROM_GETTER }));
	});

	it("falls back and uses COORDINATOR() when getCoordinator() fails in fallback", async () => {
		const provider = makeProvider({
			multicallImpl: async () => {
				throw noMulticall();
			},
			readContractImpl: async ({ functionName }: { functionName: string }) => {
				if (functionName === "getCoordinator") throw new Error("not found");
				return COORDINATOR_FROM_UPPER;
			},
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });

		expect(provider.getLogs).toHaveBeenCalledWith(expect.objectContaining({ address: COORDINATOR_FROM_UPPER }));
	});

	it("throws when both fallback readContract calls fail", async () => {
		const provider = makeProvider({
			multicallImpl: async () => {
				throw noMulticall();
			},
			readContractImpl: async () => {
				throw new Error("not found");
			},
		});

		const load = await loadModule();
		await expect(load({ provider, ...baseArgs })).rejects.toThrow(
			`Could not read coordinator from consensus contract ${CONSENSUS}`,
		);
	});

	it("caches the coordinator and does not call the provider again for the same consensus", async () => {
		const provider = makeProvider({
			multicallImpl: async () => [
				{ status: "failure", error: new Error("not found") },
				{ status: "success", result: COORDINATOR_FROM_GETTER },
			],
		});

		const load = await loadModule();
		await load({ provider, ...baseArgs });
		await load({ provider, ...baseArgs });

		// multicall should only be called once despite two loadLatestAttestationStatus invocations
		expect(provider.multicall).toHaveBeenCalledTimes(1);
	});

	it("does not cache failures — retries on next call", async () => {
		let callCount = 0;
		const provider = makeProvider({
			multicallImpl: async () => {
				callCount++;
				if (callCount === 1) {
					return [
						{ status: "failure", error: new Error("not found") },
						{ status: "failure", error: new Error("not found") },
					];
				}
				// Second attempt succeeds
				return [
					{ status: "failure", error: new Error("not found") },
					{ status: "success", result: COORDINATOR_FROM_GETTER },
				];
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
