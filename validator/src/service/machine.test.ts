import { zeroAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import { TEST_POINT } from "../__tests__/data/machine.js";
import type { KeyGenClient } from "../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../consensus/protocol/types.js";
import type { SigningClient } from "../consensus/signing/client.js";
import type { Participant } from "../consensus/storage/types.js";
import type { VerificationEngine } from "../consensus/verify/engine.js";
import type { StateStorage } from "../machine/storage/types.js";
import type { ConsensusState, MachineStates, StateDiff } from "../machine/types.js";
import type { Logger } from "../utils/logging.js";
import type { Metrics } from "../utils/metrics/index.js";
import { SafenetStateMachine } from "./machine.js";

// --- Test Data ---

const PARTICIPANTS: Participant[] = [
	{ id: 1n, address: zeroAddress },
	{ id: 3n, address: zeroAddress },
	{ id: 7n, address: zeroAddress },
];

const makeLogger = (): Logger =>
	({
		error: vi.fn(),
		warn: vi.fn(),
		notice: vi.fn(),
		info: vi.fn(),
		debug: vi.fn(),
		silly: vi.fn(),
	}) as unknown as Logger;

const makeMetrics = (): Metrics =>
	({
		transitions: { labels: vi.fn().mockReturnValue({ inc: vi.fn() }) },
		blockNumber: { set: vi.fn() },
		eventIndex: { set: vi.fn() },
		frostGroupCleanups: { labels: vi.fn().mockReturnValue({ inc: vi.fn() }) },
	}) as unknown as Metrics;

const makeStorage = (
	epochGroups: ConsensusState["epochGroups"],
	rolloverState: MachineStates["rollover"],
	activeEpoch = 0n,
): StateStorage => ({
	applyDiff: vi.fn().mockImplementation((diff: StateDiff) => {
		// Simulate the real applyConsensus behaviour: delete epoch group entries by key.
		// This ensures tests catch any code that reads epochGroups after applyDiff runs.
		for (const epoch of diff.consensus?.removeEpochGroups ?? []) {
			delete (epochGroups as Record<string, unknown>)[epoch.toString()];
		}
		return [];
	}),
	consensusState: vi.fn().mockReturnValue({
		epochGroups,
		activeEpoch,
		groupPendingNonces: {},
		signatureIdToMessage: {},
	}),
	machineStates: vi.fn().mockReturnValue({ rollover: rolloverState, signing: {} }),
});

const makeKeyGenClient = (unregisterGroup: ReturnType<typeof vi.fn>): KeyGenClient =>
	({
		setupGroup: vi.fn().mockReturnValue({
			groupId: "0xnewgroup",
			participantsRoot: "0xabc",
			participantId: 3n,
			commitments: [TEST_POINT],
			encryptionPublicKey: TEST_POINT,
			pok: { r: TEST_POINT, mu: 100n },
			poap: ["0xpoap"],
		}),
		unregisterGroup,
	}) as unknown as KeyGenClient;

const makeProtocol = (): SafenetProtocol =>
	({
		consensus: vi.fn().mockReturnValue(zeroAddress),
		process: vi.fn(),
	}) as unknown as SafenetProtocol;

const makeMachine = (
	storage: StateStorage,
	keyGenClient: KeyGenClient,
	logger: Logger,
	metrics: Metrics,
): SafenetStateMachine =>
	new SafenetStateMachine({
		participants: PARTICIPANTS,
		genesisSalt: zeroHash,
		protocol: makeProtocol(),
		keyGenClient,
		signingClient: {} as unknown as SigningClient,
		verificationEngine: {} as unknown as VerificationEngine,
		logger,
		metrics,
		blocksPerEpoch: 10n,
		keyGenTimeout: 20n,
		signingTimeout: 20n,
		storage,
	});

// Flush all pending microtasks and the next macrotask tick
const flushAsync = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

// --- Tests ---

describe("SafenetStateMachine FROST group cleanup", () => {
	it("should call unregisterGroup with the correct group IDs on epoch rollover", async () => {
		// Epochs 1, 3, 5 exist; activating epoch 5 → activeEpoch = 3, epochCutoff = 3
		// epoch "1" (1 < 3) should be cleaned up; epoch "3" and "5" preserved
		const epochGroups = {
			"1": { groupId: "0xgroup1" as const, participantId: 1n },
			"3": { groupId: "0xgroup3" as const, participantId: 1n },
			"5": { groupId: "0xgroup5" as const, participantId: 1n },
		};
		const storage = makeStorage(epochGroups, { id: "epoch_staged", nextEpoch: 5n }, 3n);
		const unregisterGroup = vi.fn();
		const keyGenClient = makeKeyGenClient(unregisterGroup);
		const metrics = makeMetrics();
		const machine = makeMachine(storage, keyGenClient, makeLogger(), metrics);

		// blocksPerEpoch = 10, block 50 → currentEpoch = 5
		machine.transition({ id: "block_new", block: 50n });
		await flushAsync();

		expect(unregisterGroup).toHaveBeenCalledOnce();
		expect(unregisterGroup).toHaveBeenCalledWith("0xgroup1");
		expect(unregisterGroup).not.toHaveBeenCalledWith("0xgroup3");
		expect(unregisterGroup).not.toHaveBeenCalledWith("0xgroup5");
		expect(metrics.frostGroupCleanups.labels).toHaveBeenCalledWith({ result: "success" });
		expect(metrics.frostGroupCleanups.labels).not.toHaveBeenCalledWith({ result: "failure" });
	});

	it("should tolerate unregisterGroup failure, log a warning, and increment the metric", async () => {
		const epochGroups = {
			"1": { groupId: "0xgroup1" as const, participantId: 1n },
			"3": { groupId: "0xgroup3" as const, participantId: 1n },
			"5": { groupId: "0xgroup5" as const, participantId: 1n },
		};
		const storage = makeStorage(epochGroups, { id: "epoch_staged", nextEpoch: 5n }, 3n);
		const unregisterGroup = vi.fn().mockImplementation(() => {
			throw new Error("crypto DB unavailable");
		});
		const keyGenClient = makeKeyGenClient(unregisterGroup);
		const logger = makeLogger();
		const metrics = makeMetrics();
		const machine = makeMachine(storage, keyGenClient, logger, metrics);

		machine.transition({ id: "block_new", block: 50n });
		await flushAsync();

		expect(unregisterGroup).toHaveBeenCalledOnce();
		expect(metrics.frostGroupCleanups.labels).toHaveBeenCalledWith({ result: "failure" });
		expect(metrics.frostGroupCleanups.labels).not.toHaveBeenCalledWith({ result: "success" });
		expect(logger.warn).toHaveBeenCalledWith(
			expect.stringContaining("0xgroup1"),
			expect.objectContaining({ error: expect.any(Error) }),
		);
	});

	it("should not call unregisterGroup when removeEpochGroups is absent", async () => {
		// waiting_for_genesis → checkEpochRollover returns {} immediately
		const storage = makeStorage({}, { id: "waiting_for_genesis" });
		const unregisterGroup = vi.fn();
		const keyGenClient = makeKeyGenClient(unregisterGroup);
		const machine = makeMachine(storage, keyGenClient, makeLogger(), makeMetrics());

		machine.transition({ id: "block_new", block: 10n });
		await flushAsync();

		expect(unregisterGroup).not.toHaveBeenCalled();
	});
});
