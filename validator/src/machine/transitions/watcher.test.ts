import Sqlite3 from "better-sqlite3";
import type { PublicClient } from "viem";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Logger } from "../../utils/logging.js";
import type { Metrics } from "../../utils/metrics.js";
import { watchBlocksAndEvents } from "../../watcher/index.js";
import { OnchainTransitionWatcher, type WatcherConfig } from "./watcher.js";

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
	return watcher;
};

// `blockTimeOverride` avoids needing a chain block time on the stubbed client.
const baseConfig: WatcherConfig = { maxReorgDepth: 5, blockTimeOverride: 5000 };

const startedWithBlock = () => mockWatch.mock.calls.at(-1)?.[0].lastIndexedBlock;

describe("OnchainTransitionWatcher initial block resolution", () => {
	beforeEach(() => {
		mockWatch.mockClear();
	});

	it("uses the configured start block when there is no persisted state", async () => {
		const watcher = createWatcher({ ...baseConfig, startFromBlock: 456n });
		await watcher.start();
		expect(startedWithBlock()).toBe(456n);
	});

	it("prefers persisted indexing state over the configured start block", async () => {
		const watcher = createWatcher({ ...baseConfig, startFromBlock: 456n });
		watcher.updateLastIndexedBlock(123n);
		await watcher.start();
		expect(startedWithBlock()).toBe(123n);
	});

	it("defaults to null when neither persisted state nor a start block is set", async () => {
		const watcher = createWatcher(baseConfig);
		await watcher.start();
		expect(startedWithBlock()).toBeNull();
	});
});
