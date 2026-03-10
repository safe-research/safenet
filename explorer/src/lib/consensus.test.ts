import type { Address, Hex, PublicClient } from "viem";
import { describe, expect, it, vi } from "vitest";
import { loadEpochRolloverHistory, loadEpochsState } from "./consensus";

const CONSENSUS = "0x1111111111111111111111111111111111111111" as Address;
const GROUP_ID_A = `0x${"aa".repeat(32)}` as Hex;
const GROUP_ID_B = `0x${"bb".repeat(32)}` as Hex;

const makeStagedLog = ({
	activeEpoch,
	proposedEpoch,
	rolloverBlock,
	groupId,
	blockNumber,
	logIndex = 0,
}: {
	activeEpoch: bigint;
	proposedEpoch: bigint;
	rolloverBlock: bigint;
	groupId: Hex;
	blockNumber: bigint;
	logIndex?: number;
}) => ({
	args: {
		activeEpoch,
		proposedEpoch,
		rolloverBlock,
		groupId,
		groupKey: { x: 1n, y: 2n },
		signatureId: `0x${"00".repeat(32)}`,
		attestation: { r: { x: 0n, y: 0n }, z: 0n },
	},
	blockNumber,
	logIndex,
	transactionHash: `0x${"00".repeat(32)}`,
	blockHash: `0x${"00".repeat(32)}`,
	address: CONSENSUS,
	data: "0x",
	topics: [],
	transactionIndex: 0,
	removed: false,
});

describe("loadEpochRolloverHistory", () => {
	const makeProvider = ({
		blockNumber = 1000n,
		logs = [],
	}: {
		blockNumber?: bigint;
		logs?: ReturnType<typeof makeStagedLog>[];
	} = {}): PublicClient =>
		({
			getBlockNumber: vi.fn().mockResolvedValue(blockNumber),
			getLogs: vi.fn().mockResolvedValue(logs),
		}) as unknown as PublicClient;

	it("returns empty entries when no logs are found", async () => {
		const result = await loadEpochRolloverHistory({
			provider: makeProvider(),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toEqual([]);
		expect(result.reachedGenesis).toBe(false);
	});

	it("maps EpochStaged logs to rollover entries", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 150n,
				groupId: GROUP_ID_A,
				blockNumber: 200n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toHaveLength(1);
		expect(result.entries[0]).toEqual({
			activeEpoch: 1n,
			proposedEpoch: 2n,
			rolloverBlock: 150n,
			groupId: GROUP_ID_A,
			stagedAt: 200n,
		});
	});

	it("detects genesis when activeEpoch is 0", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 0n,
				proposedEpoch: 1n,
				rolloverBlock: 10n,
				groupId: GROUP_ID_A,
				blockNumber: 50n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.reachedGenesis).toBe(true);
	});

	it("does not detect genesis when only proposedEpoch is 0", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 1n,
				proposedEpoch: 0n,
				rolloverBlock: 10n,
				groupId: GROUP_ID_A,
				blockNumber: 50n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.reachedGenesis).toBe(false);
	});

	it("returns entries sorted most recent first", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 100n,
				groupId: GROUP_ID_A,
				blockNumber: 100n,
			}),
			makeStagedLog({
				activeEpoch: 2n,
				proposedEpoch: 3n,
				rolloverBlock: 200n,
				groupId: GROUP_ID_B,
				blockNumber: 200n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toHaveLength(2);
		expect(result.entries[0].proposedEpoch).toBe(3n);
		expect(result.entries[1].proposedEpoch).toBe(2n);
	});

	it("uses cursor as toBlock when provided", async () => {
		const provider = makeProvider();
		await loadEpochRolloverHistory({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: 500n,
			cursor: 800n,
		});
		expect(provider.getLogs).toHaveBeenCalledWith(
			expect.objectContaining({
				toBlock: 800n,
			}),
		);
		expect(provider.getBlockNumber).not.toHaveBeenCalled();
	});

	it("uses 'latest' as toBlock when no cursor is provided", async () => {
		const provider = makeProvider();
		await loadEpochRolloverHistory({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(provider.getLogs).toHaveBeenCalledWith(
			expect.objectContaining({
				toBlock: "latest",
			}),
		);
	});
});

describe("loadEpochsState", () => {
	it("returns epoch state with group IDs", async () => {
		const provider = {
			readContract: vi.fn().mockImplementation(({ functionName, args }: { functionName: string; args?: unknown[] }) => {
				if (functionName === "getEpochsState") {
					return [1n, 2n, 3n, 500n];
				}
				if (functionName === "getEpochGroupId") {
					const epoch = (args as bigint[])[0];
					if (epoch === 2n) return GROUP_ID_A;
					if (epoch === 3n) return GROUP_ID_B;
				}
				return undefined;
			}),
		} as unknown as PublicClient;

		const state = await loadEpochsState(provider, CONSENSUS);
		expect(state).toEqual({
			previous: 1n,
			active: 2n,
			staged: 3n,
			rolloverBlock: 500n,
			activeGroupId: GROUP_ID_A,
			stagedGroupId: GROUP_ID_B,
		});
	});

	it("returns null stagedGroupId when staged epoch is 0", async () => {
		const provider = {
			readContract: vi.fn().mockImplementation(({ functionName, args }: { functionName: string; args?: unknown[] }) => {
				if (functionName === "getEpochsState") {
					return [0n, 1n, 0n, 0n];
				}
				if (functionName === "getEpochGroupId") {
					const epoch = (args as bigint[])[0];
					if (epoch === 1n) return GROUP_ID_A;
				}
				return undefined;
			}),
		} as unknown as PublicClient;

		const state = await loadEpochsState(provider, CONSENSUS);
		expect(state.stagedGroupId).toBeNull();
		expect(state.activeGroupId).toBe(GROUP_ID_A);
	});
});
