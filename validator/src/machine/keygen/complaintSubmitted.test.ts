import { ethAddress, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import { toPoint } from "../../frost/math.js";
import type { KeyGenComplaintSubmittedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates } from "../types.js";
import { handleComplaintSubmitted } from "./complaintSubmitted.js";
import { calcGroupContext } from "./group.js";

// --- Test Data ---
const EVENT: KeyGenComplaintSubmittedEvent = {
	id: "event_key_gen_complaint_submitted",
	block: 21n,
	index: 0,
	gid: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
	plaintiff: 1n,
	accused: 2n,
};
const TEST_POINT = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});
const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [
		{ id: 1n, address: entryPoint06Address },
		{ id: 2n, address: entryPoint07Address },
		{ id: 3n, address: entryPoint08Address },
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 10n,
	signingTimeout: 20n,
	blocksPerEpoch: 10n,
};

// --- Tests ---
describe("complaint submitted", () => {
	it("should not handle event if in unexpected state", async () => {
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_commitments",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle complaint if unexpected group id", async () => {
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x5afe5afe",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				complaints: {},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle complaint in collecting confirmations if complaint deadline has passed", async () => {
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 20n,
				responseDeadline: 25n,
				deadline: 30n,
				complaints: {},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should accept complaints when collecting shares", async () => {
		const participantId = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
			participantId,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
		});
	});

	it("should accept complaints when collecting confirmations", async () => {
		const participantId = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
			participantId,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				complaints: {},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
		});
	});

	it("should accept multiple complaints for different accused", async () => {
		const participantId = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
			participantId,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"1": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"1": { unresponded: 1n, total: 1n },
					"2": { unresponded: 1n, total: 1n },
				},
			},
		});
	});

	it("should accept multiple complaints for same accused", async () => {
		const participantId = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
			participantId,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { unresponded: 2n, total: 2n },
				},
			},
		});
	});

	it("should immediately react to complaint when accused", async () => {
		const participantId = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participantId.mockReturnValueOnce(2n);
		const secretShare = 0x5afe5afe5afen;
		const createSecretShare = vi.fn();
		createSecretShare.mockReturnValueOnce(secretShare);
		const keyGenClient = {
			createSecretShare,
			participantId,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = { consensus: vi.fn().mockReturnValue(ethAddress) } as unknown as ShieldnetProtocol;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { unresponded: 2n, total: 2n },
				},
			},
			actions: [
				{
					id: "key_gen_complaint_response",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					plaintiff: 1n,
					secretShare,
				},
			],
		});
	});

	it("should restart key gen when complaints exceed threshold", async () => {
		const groupSetup = {
			groupId: "0x5afe02",
			participantsRoot: "0x5afe5afe5afe",
			participantId: 1n,
			commitments: [TEST_POINT],
			pok: {
				r: TEST_POINT,
				mu: 100n,
			},
			poap: ["0x5afe5afe5afe01"],
		};
		const participants = [
			{ id: 1n, address: entryPoint06Address },
			{ id: 2n, address: entryPoint07Address },
			{ id: 3n, address: entryPoint08Address },
		];
		const setupGroup = vi.fn();
		setupGroup.mockReturnValueOnce(groupSetup);
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(2n);
		const keyGenClient = {
			setupGroup,
			threshold,
			participants: vi.fn().mockReturnValueOnce(participants),
		} as unknown as KeyGenClient;
		const consensus = vi.fn();
		consensus.mockReturnValueOnce(ethAddress);
		const protocol = { consensus } as unknown as ShieldnetProtocol;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: EVENT.gid,
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { unresponded: 0n, total: 1n },
				},
			},
			signing: {},
		};

		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);

		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 2n,
				threshold: 2n,
				context: calcGroupContext(ethAddress, 10n),
				participantId: 1n,
				commitments: groupSetup.commitments,
				pok: groupSetup.pok,
				poap: groupSetup.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x5afe02",
			nextEpoch: 10n,
			deadline: 31n,
		});
		expect(diff.consensus).toStrictEqual({
			epochGroup: [10n, { groupId: "0x5afe02", participantId: 1n }],
		});
		expect(consensus).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith([participants[0], participants[2]], 2n, 2n, calcGroupContext(ethAddress, 10n));
	});
});
