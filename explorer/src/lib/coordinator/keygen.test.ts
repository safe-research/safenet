import {
	type Address,
	encodeAbiParameters,
	encodeEventTopics,
	getAbiItem,
	type Hex,
	numberToHex,
	type PublicClient,
} from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { COORDINATOR_KEY_GEN_EVENTS } from "./abi";

// biome-ignore lint/suspicious/noExplicitAny: viem's ABI input types don't always include the `indexed` property
const nonIndexedInputs = (inputs: readonly any[]) => inputs.filter((i: any) => !i.indexed);

const CONSENSUS = "0x1111111111111111111111111111111111111111" as Address;
const COORDINATOR = "0x2222222222222222222222222222222222222222" as Address;
const GID = `0x${"aa".repeat(32)}` as Hex;
const CONTEXT = `0x${"cc".repeat(32)}` as Hex;

const PARTICIPANT_1 = "0x0000000000000000000000000000000000000001" as Address;
const PARTICIPANT_2 = "0x0000000000000000000000000000000000000002" as Address;
const PARTICIPANT_3 = "0x0000000000000000000000000000000000000003" as Address;

type LoadKeyGenDetails = typeof import("./keygen").loadKeyGenDetails;

const loadModule = async () => {
	vi.resetModules();
	const mod = await import("./keygen");
	return mod.loadKeyGenDetails as LoadKeyGenDetails;
};

const zeroPoint = { x: 0n, y: 0n };

const encodeKeyGenLog = (blockNumber: bigint, logIndex = 0, count = 3, threshold = 2) => {
	const topics = encodeEventTopics({
		abi: COORDINATOR_KEY_GEN_EVENTS,
		eventName: "KeyGen",
		args: { gid: GID, context: CONTEXT },
	});
	const data = encodeAbiParameters(
		[{ type: "bytes32" }, { type: "uint16" }, { type: "uint16" }],
		[`0x${"dd".repeat(32)}`, count, threshold],
	);
	return {
		address: COORDINATOR,
		topics,
		data,
		blockNumber: numberToHex(blockNumber),
		logIndex: numberToHex(logIndex),
		transactionHash: `0x${"00".repeat(32)}`,
		blockHash: `0x${"00".repeat(32)}`,
		transactionIndex: "0x0",
		removed: false,
	};
};

const encodeKeyGenCommittedLog = (blockNumber: bigint, participant: Address, logIndex = 1) => {
	const topics = encodeEventTopics({
		abi: COORDINATOR_KEY_GEN_EVENTS,
		eventName: "KeyGenCommitted",
		args: { gid: GID },
	});
	const commitment = { q: zeroPoint, c: [zeroPoint], r: zeroPoint, mu: 0n };
	const data = encodeAbiParameters(
		nonIndexedInputs(getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenCommitted" }).inputs),
		[participant, commitment, true],
	);
	return {
		address: COORDINATOR,
		topics,
		data,
		blockNumber: numberToHex(blockNumber),
		logIndex: numberToHex(logIndex),
		transactionHash: `0x${"00".repeat(32)}`,
		blockHash: `0x${"00".repeat(32)}`,
		transactionIndex: "0x0",
		removed: false,
	};
};

const encodeKeyGenSecretSharedLog = (blockNumber: bigint, participant: Address, logIndex = 2) => {
	const topics = encodeEventTopics({
		abi: COORDINATOR_KEY_GEN_EVENTS,
		eventName: "KeyGenSecretShared",
		args: { gid: GID },
	});
	const share = { y: zeroPoint, f: [0n] };
	const data = encodeAbiParameters(
		nonIndexedInputs(getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenSecretShared" }).inputs),
		[participant, share, true],
	);
	return {
		address: COORDINATOR,
		topics,
		data,
		blockNumber: numberToHex(blockNumber),
		logIndex: numberToHex(logIndex),
		transactionHash: `0x${"00".repeat(32)}`,
		blockHash: `0x${"00".repeat(32)}`,
		transactionIndex: "0x0",
		removed: false,
	};
};

const encodeKeyGenConfirmedLog = (blockNumber: bigint, participant: Address, logIndex = 3) => {
	const topics = encodeEventTopics({
		abi: COORDINATOR_KEY_GEN_EVENTS,
		eventName: "KeyGenConfirmed",
		args: { gid: GID },
	});
	const data = encodeAbiParameters(
		nonIndexedInputs(getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenConfirmed" }).inputs),
		[participant, true],
	);
	return {
		address: COORDINATOR,
		topics,
		data,
		blockNumber: numberToHex(blockNumber),
		logIndex: numberToHex(logIndex),
		transactionHash: `0x${"00".repeat(32)}`,
		blockHash: `0x${"00".repeat(32)}`,
		transactionIndex: "0x0",
		removed: false,
	};
};

const encodeKeyGenComplainedLog = (
	blockNumber: bigint,
	plaintiff: Address,
	accused: Address,
	compromised: boolean,
	logIndex = 4,
) => {
	const topics = encodeEventTopics({
		abi: COORDINATOR_KEY_GEN_EVENTS,
		eventName: "KeyGenComplained",
		args: { gid: GID },
	});
	const data = encodeAbiParameters(
		nonIndexedInputs(getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenComplained" }).inputs),
		[plaintiff, accused, compromised],
	);
	return {
		address: COORDINATOR,
		topics,
		data,
		blockNumber: numberToHex(blockNumber),
		logIndex: numberToHex(logIndex),
		transactionHash: `0x${"00".repeat(32)}`,
		blockHash: `0x${"00".repeat(32)}`,
		transactionIndex: "0x0",
		removed: false,
	};
};

const makeProvider = (rpcLogs: unknown[] = []): PublicClient =>
	({
		readContract: vi.fn().mockResolvedValue(COORDINATOR),
		request: vi.fn().mockResolvedValue(rpcLogs),
	}) as unknown as PublicClient;

const baseArgs = {
	consensus: CONSENSUS,
	gid: GID,
	endBlock: 1000n,
	maxBlockRange: 500n,
};

describe("loadKeyGenDetails", () => {
	beforeEach(() => {
		vi.resetModules();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("returns null when no KeyGen event is found", async () => {
		const load = await loadModule();
		const result = await load({ provider: makeProvider([]), ...baseArgs });
		expect(result).toBeNull();
	});

	it("returns basic keygen status from KeyGen event", async () => {
		const logs = [encodeKeyGenLog(900n)];
		const load = await loadModule();
		const result = await load({ provider: makeProvider(logs), ...baseArgs });

		expect(result).toEqual(
			expect.objectContaining({
				gid: GID,
				count: 3,
				threshold: 2,
				committed: [],
				shared: [],
				confirmed: [],
				finalized: false,
				compromised: false,
			}),
		);
	});

	it("tracks committed participants", async () => {
		const logs = [
			encodeKeyGenLog(900n),
			encodeKeyGenCommittedLog(910n, PARTICIPANT_1),
			encodeKeyGenCommittedLog(920n, PARTICIPANT_2, 2),
		];
		const load = await loadModule();
		const result = await load({ provider: makeProvider(logs), ...baseArgs });

		expect(result).toEqual(
			expect.objectContaining({
				committed: [
					{ address: PARTICIPANT_1, block: 910n },
					{ address: PARTICIPANT_2, block: 920n },
				],
			}),
		);
	});

	it("tracks shared participants", async () => {
		const logs = [
			encodeKeyGenLog(900n),
			encodeKeyGenSecretSharedLog(930n, PARTICIPANT_1),
			encodeKeyGenSecretSharedLog(940n, PARTICIPANT_2, 3),
		];
		const load = await loadModule();
		const result = await load({ provider: makeProvider(logs), ...baseArgs });

		expect(result).toEqual(
			expect.objectContaining({
				shared: [
					{ address: PARTICIPANT_1, block: 930n },
					{ address: PARTICIPANT_2, block: 940n },
				],
			}),
		);
	});

	it("marks as finalized when confirmed count meets threshold", async () => {
		const logs = [
			encodeKeyGenLog(900n, 0, 3, 2),
			encodeKeyGenConfirmedLog(950n, PARTICIPANT_1),
			encodeKeyGenConfirmedLog(960n, PARTICIPANT_2, 4),
		];
		const load = await loadModule();
		const result = await load({ provider: makeProvider(logs), ...baseArgs });

		expect(result).toEqual(
			expect.objectContaining({
				confirmed: expect.arrayContaining([expect.objectContaining({ address: PARTICIPANT_1 })]),
				finalized: true,
				compromised: false,
			}),
		);
	});

	it("does not mark as finalized when confirmed count is below threshold", async () => {
		const logs = [encodeKeyGenLog(900n, 0, 3, 2), encodeKeyGenConfirmedLog(950n, PARTICIPANT_1)];
		const load = await loadModule();
		const result = await load({ provider: makeProvider(logs), ...baseArgs });

		expect(result).toEqual(
			expect.objectContaining({
				finalized: false,
			}),
		);
	});

	it("marks as compromised and not finalized when complaint succeeds", async () => {
		const logs = [
			encodeKeyGenLog(900n, 0, 3, 2),
			encodeKeyGenComplainedLog(940n, PARTICIPANT_1, PARTICIPANT_2, true),
			encodeKeyGenConfirmedLog(950n, PARTICIPANT_1, 5),
			encodeKeyGenConfirmedLog(960n, PARTICIPANT_3, 6),
		];
		const load = await loadModule();
		const result = await load({ provider: makeProvider(logs), ...baseArgs });

		expect(result).toEqual(
			expect.objectContaining({
				compromised: true,
				finalized: false,
			}),
		);
	});

	it("ignores non-compromising complaints", async () => {
		const logs = [
			encodeKeyGenLog(900n, 0, 3, 2),
			encodeKeyGenComplainedLog(940n, PARTICIPANT_1, PARTICIPANT_2, false),
			encodeKeyGenConfirmedLog(950n, PARTICIPANT_1, 5),
			encodeKeyGenConfirmedLog(960n, PARTICIPANT_3, 6),
		];
		const load = await loadModule();
		const result = await load({ provider: makeProvider(logs), ...baseArgs });

		expect(result).toEqual(
			expect.objectContaining({
				compromised: false,
				finalized: true,
			}),
		);
	});

	it("uses blocksPerEpoch to compute startBlock", async () => {
		const provider = makeProvider([]);
		const load = await loadModule();
		await load({ provider, ...baseArgs, endBlock: 1050n, blocksPerEpoch: 100 });

		expect(provider.request).toHaveBeenCalledWith(
			expect.objectContaining({
				params: [
					expect.objectContaining({
						fromBlock: numberToHex(1000n),
					}),
				],
			}),
		);
	});

	it("uses prevStagedAt as startBlock when blocksPerEpoch is not set", async () => {
		const provider = makeProvider([]);
		const load = await loadModule();
		await load({ provider, ...baseArgs, prevStagedAt: 800n });

		expect(provider.request).toHaveBeenCalledWith(
			expect.objectContaining({
				params: [
					expect.objectContaining({
						fromBlock: numberToHex(800n),
					}),
				],
			}),
		);
	});

	it("falls back to maxBlockRange for startBlock computation", async () => {
		const provider = makeProvider([]);
		const load = await loadModule();
		await load({ provider, ...baseArgs, endBlock: 1000n, maxBlockRange: 300n });

		expect(provider.request).toHaveBeenCalledWith(
			expect.objectContaining({
				params: [
					expect.objectContaining({
						fromBlock: numberToHex(700n),
					}),
				],
			}),
		);
	});

	it("clamps startBlock to 0 when endBlock is less than maxBlockRange", async () => {
		const provider = makeProvider([]);
		const load = await loadModule();
		await load({ provider, ...baseArgs, endBlock: 50n, maxBlockRange: 500n });

		expect(provider.request).toHaveBeenCalledWith(
			expect.objectContaining({
				params: [
					expect.objectContaining({
						fromBlock: numberToHex(0n),
					}),
				],
			}),
		);
	});
});
