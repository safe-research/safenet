import { ethAddress, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { KeyGenComplaintResponsedEvent as KeyGenComplaintRespondedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates } from "../types.js";
import { handleComplaintResponded } from "./complaintResponse.js";

// --- Test Data ---
const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});
const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [
		{
			id: 1n,
			address: entryPoint06Address,
		},
		{
			id: 3n,
			address: entryPoint07Address,
		},
		{
			id: 2n,
			address: entryPoint08Address,
		},
		{
			id: 4n,
			address: ethAddress,
		},
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 15n,
	signingTimeout: 20n,
	blocksPerEpoch: 10n,
};

const EVENT: KeyGenComplaintRespondedEvent = {
	id: "event_key_gen_complaint_responded",
	block: 21n,
	index: 0,
	gid: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
	plaintiff: 1n,
	accused: 2n,
	secretShare: 0x5afe5afe5afen,
};

// --- Tests ---
describe("complaint responded", () => {
	it("should not handle responses if in unexpected state", async () => {
		const protocol = {} as unknown as SafenetProtocol;
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
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle responses if unexpected group id", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x5afe5afe",
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
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle responses in collecting confirmations if response deadline has passed", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 10n,
				responseDeadline: 20n,
				deadline: 30n,
				complaints: {},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle responses if no complaints tracked", async () => {
		const protocol = {} as unknown as SafenetProtocol;
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
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should accept responses when collecting shares", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const verifySecretShare = vi.fn();
		verifySecretShare.mockReturnValueOnce(true);
		const keyGenClient = {
			verifySecretShare,
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { total: 1n, unresponded: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					"2": { unresponded: 0n, total: 1n },
				},
			},
			actions: [],
		});
	});

	it("should accept complaints when collecting confirmations", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const verifySecretShare = vi.fn();
		verifySecretShare.mockReturnValueOnce(true);
		const keyGenClient = {
			verifySecretShare,
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				complaints: {
					"2": { total: 1n, unresponded: 1n },
				},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				complaints: {
					"2": { unresponded: 0n, total: 1n },
				},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
			actions: [],
		});
	});

	it("should trigger key gen on invalid response for other plaintiff", async () => {
		const consensus = vi.fn();
		consensus.mockReturnValueOnce(ethAddress);
		const protocol = {
			consensus,
		} as unknown as SafenetProtocol;
		const groupSetup = {
			groupId: "0x5afe02",
			participantsRoot: "0x5afe5afe5afe",
			participantId: 4n,
			commitments: [TEST_POINT],
			pok: {
				r: TEST_POINT,
				mu: 100n,
			},
			poap: ["0x5afe5afe5afe01"],
		};
		const setupGroup = vi.fn();
		setupGroup.mockReturnValueOnce(groupSetup);
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(2n);
		const verifySecretShare = vi.fn();
		verifySecretShare.mockReturnValueOnce(false);
		const participants = vi.fn();
		participants.mockReturnValueOnce(MACHINE_CONFIG.defaultParticipants);
		const keyGenClient = {
			participants,
			setupGroup,
			verifySecretShare,
			participantId,
		} as unknown as KeyGenClient;
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
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_commitments",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				deadline: 36n,
			},
			consensus: {
				epochGroup: [10n, { groupId: "0x5afe02", participantId: 4n }],
			},
			actions: [
				{
					id: "key_gen_start",
					participants: groupSetup.participantsRoot,
					count: 3,
					threshold: 2,
					context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
					participantId: 4n,
					commitments: groupSetup.commitments,
					pok: groupSetup.pok,
					poap: groupSetup.poap,
				},
			],
		});
	});

	it("should trigger key gen on invalid response for self as plaintiff", async () => {
		const consensus = vi.fn();
		consensus.mockReturnValueOnce(ethAddress);
		const protocol = {
			consensus,
		} as unknown as SafenetProtocol;
		const groupSetup = {
			groupId: "0x5afe02",
			participantsRoot: "0x5afe5afe5afe",
			participantId: 4n,
			commitments: [TEST_POINT],
			pok: {
				r: TEST_POINT,
				mu: 100n,
			},
			poap: ["0x5afe5afe5afe01"],
		};
		const setupGroup = vi.fn();
		setupGroup.mockReturnValueOnce(groupSetup);
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const participants = vi.fn();
		participants.mockReturnValueOnce(MACHINE_CONFIG.defaultParticipants);
		const registerPlainKeyGenSecret = vi.fn();
		registerPlainKeyGenSecret.mockReturnValueOnce("invalid_share");
		const keyGenClient = {
			participants,
			setupGroup,
			registerPlainKeyGenSecret,
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [2n],
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_commitments",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				deadline: 36n,
			},
			consensus: {
				epochGroup: [10n, { groupId: "0x5afe02", participantId: 4n }],
			},
			actions: [
				{
					id: "key_gen_start",
					participants: groupSetup.participantsRoot,
					count: 3,
					threshold: 2,
					context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
					participantId: 4n,
					commitments: groupSetup.commitments,
					pok: groupSetup.pok,
					poap: groupSetup.poap,
				},
			],
		});
	});

	it("should remove missing share once received", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const registerPlainKeyGenSecret = vi.fn();
		registerPlainKeyGenSecret.mockReturnValueOnce("pending_shares");
		const keyGenClient = {
			registerPlainKeyGenSecret,
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				missingSharesFrom: [2n],
				confirmationsFrom: [],
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				missingSharesFrom: [],
				confirmationsFrom: [],
				complaints: {
					"2": { unresponded: 0n, total: 1n },
				},
			},
			actions: [],
		});
	});

	it("should trigger confirmation if missing share in collecting confirmations", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const registerPlainKeyGenSecret = vi.fn();
		registerPlainKeyGenSecret.mockReturnValueOnce("shares_completed");
		const keyGenClient = {
			registerPlainKeyGenSecret,
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				missingSharesFrom: [2n],
				confirmationsFrom: [],
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintResponded(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				missingSharesFrom: [],
				confirmationsFrom: [],
				complaints: {
					"2": { unresponded: 0n, total: 1n },
				},
			},
			actions: [
				{
					id: "key_gen_confirm",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					callbackContext:
						"0x000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000064",
				},
			],
		});
	});
});
