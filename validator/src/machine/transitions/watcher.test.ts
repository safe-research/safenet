import Sqlite3 from "better-sqlite3";
import type { PublicClient } from "viem";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WatcherConfig } from "../../shared/watcher.js";
import type { Logger } from "../../utils/logging.js";
import type { Metrics } from "../../utils/metrics.js";
import { watchBlocksAndEvents } from "../../watcher/index.js";
import { OnchainTransitionWatcher } from "./watcher.js";

vi.mock("../../watcher/index.js", () => ({
	watchBlocksAndEvents: vi.fn(async () => async () => {}),
}));

const mockWatch = vi.mocked(watchBlocksAndEvents);

const ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" as const;

const createWatcher = (watcherConfig: WatcherConfig) => {
	const database = new Sqlite3(":memory:");
	const publicClient = { chain: { id: 100 } } as unknown as PublicClient;
	const logger = { error: vi.fn(), warn: vi.fn(), notice: vi.fn(), info: vi.fn(), debug: vi.fn(), silly: vi.fn() };
	const watcher = new OnchainTransitionWatcher({
		database,
		publicClient,
		config: { consensus: ADDRESS, coordinator: ADDRESS, allowedOracles: [] },
		watcherConfig,
		logger: logger as unknown as Logger,
		metrics: {} as unknown as Metrics,
		onTransition: () => {},
	});
	return { watcher, database };
};

// `blockTimeOverride` avoids needing a chain block time on the stubbed client.
const baseConfig: WatcherConfig = { maxReorgDepth: 5, blockTimeOverride: 5000 };

const startedWith = () => {
	const params = mockWatch.mock.calls.at(-1)?.[0];
	return { lastIndexedBlock: params?.lastIndexedBlock, startBlock: params?.startBlock };
};

describe("OnchainTransitionWatcher initial block resolution", () => {
	beforeEach(() => {
		mockWatch.mockClear();
	});

	it("forwards the configured start block when there is no persisted state", async () => {
		const { watcher } = createWatcher({ ...baseConfig, startFromBlock: 456n });
		await watcher.start();
		// The start block is passed separately (not as `lastIndexedBlock`) so the watcher back-fills
		// via a warp instead of emitting a spurious reorg.
		expect(startedWith()).toEqual({ lastIndexedBlock: null, startBlock: 456n });
	});

	it("prefers persisted indexing state over the configured start block", async () => {
		const { watcher, database } = createWatcher({ ...baseConfig, startFromBlock: 456n });
		// Set the last indexed block in the database
		const stmt = database.prepare(`
			INSERT INTO transition_watcher (chainId, lastIndexedBlock)
			VALUES (@chainId, @block)
			ON CONFLICT(chainId) DO UPDATE
			SET lastIndexedBlock = excluded.lastIndexedBlock
		`);
		stmt.run({ chainId: 100, block: 123 });
		await watcher.start();
		expect(startedWith()).toEqual({ lastIndexedBlock: 123n, startBlock: null });
	});

	it("defaults to null when neither persisted state nor a start block is set", async () => {
		const { watcher } = createWatcher(baseConfig);
		await watcher.start();
		expect(startedWith()).toEqual({ lastIndexedBlock: null, startBlock: null });
	});
});
