import { ethAddress, zeroHash } from "viem";
import {
	entryPoint06Address,
	entryPoint07Address,
	entryPoint08Address,
	entryPoint09Address,
} from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import { makeGroupSetup, makeKeyGenSetup } from "../../__tests__/data/machine.js";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
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
	plaintiff: entryPoint06Address,
	accused: entryPoint07Address,
	compromised: false,
};
const MACHINE_CONFIG: MachineConfig = {
	account: entryPoint06Address,
	participantsInfo: [
		{ address: entryPoint06Address, activeFrom: 0n },
		{ address: entryPoint07Address, activeFrom: 0n },
		{ address: entryPoint08Address, activeFrom: 0n },
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 10n,
	signingTimeout: 20n,
	blocksPerEpoch: 10n,
};

const makeProtocol = (): SafenetProtocol =>
	({ consensus: vi.fn().mockReturnValue(ethAddress) }) as unknown as SafenetProtocol;

// --- Tests ---
describe("complaint submitted", () => {
	it("should not handle event if in unexpected state", async () => {
		const protocol = makeProtocol();
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
		const protocol = makeProtocol();
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
				sharesFrom: [],
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle complaint in collecting confirmations if complaint deadline has passed", async () => {
		const protocol = makeProtocol();
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
				sharesFrom: [],
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should accept complaints when collecting shares", async () => {
		const participant = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participant.mockReturnValueOnce(entryPoint06Address);
		const keyGenClient = {
			participant,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = makeProtocol();
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				sharesFrom: [],
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
				sharesFrom: [],
				complaints: {
					[entryPoint07Address]: { unresponded: 1, total: 1 },
				},
			},
		});
	});

	it("should accept complaints when collecting confirmations", async () => {
		const participant = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participant.mockReturnValueOnce(entryPoint06Address);
		const keyGenClient = {
			participant,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = makeProtocol();
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				complaints: {},
				sharesFrom: [],
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
					[entryPoint07Address]: { unresponded: 1, total: 1 },
				},
				sharesFrom: [],
				confirmationsFrom: [],
			},
		});
	});

	it("should accept multiple complaints for different accused", async () => {
		const participant = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participant.mockReturnValueOnce(entryPoint06Address);
		const keyGenClient = {
			participant,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = makeProtocol();
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				sharesFrom: [],
				complaints: {
					[entryPoint06Address]: { unresponded: 1, total: 1 },
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
				sharesFrom: [],
				complaints: {
					[entryPoint06Address]: { unresponded: 1, total: 1 },
					[entryPoint07Address]: { unresponded: 1, total: 1 },
				},
			},
		});
	});

	it("should accept multiple complaints for same accused", async () => {
		const participant = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participant.mockReturnValueOnce(entryPoint06Address);
		const keyGenClient = {
			participant,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = makeProtocol();
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				sharesFrom: [],
				complaints: {
					[entryPoint07Address]: { unresponded: 1, total: 1 },
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
				sharesFrom: [],
				complaints: {
					[entryPoint07Address]: { unresponded: 2, total: 2 },
				},
			},
		});
	});

	it("should immediately react to complaint when accused", async () => {
		const participant = vi.fn();
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(3n);
		participant.mockReturnValueOnce(entryPoint07Address);
		const secretShare = 0x5afe5afe5afen;
		const createSecretShare = vi.fn();
		createSecretShare.mockReturnValueOnce(secretShare);
		const keyGenClient = {
			createSecretShare,
			participant,
			threshold,
		} as unknown as KeyGenClient;
		const protocol = makeProtocol();
		const machineConfig = {
			...MACHINE_CONFIG,
			account: entryPoint07Address,
		};
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				sharesFrom: [],
				complaints: {
					[entryPoint07Address]: { unresponded: 1, total: 1 },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(machineConfig, protocol, keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				sharesFrom: [],
				complaints: {
					[entryPoint07Address]: { unresponded: 2, total: 2 },
				},
			},
			actions: [
				{
					id: "key_gen_complaint_response",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					plaintiff: entryPoint06Address,
					secretShare,
				},
			],
		});
	});

	it("should restart key gen when complaints exceed threshold", async () => {
		const groupSetup = makeGroupSetup();
		const keyGenSetup = makeKeyGenSetup();
		const participants = [entryPoint06Address, entryPoint07Address, entryPoint08Address, entryPoint09Address];
		const setupGroup = vi.fn();
		setupGroup.mockReturnValueOnce(groupSetup);
		const setupKeyGen = vi.fn();
		setupKeyGen.mockReturnValueOnce(keyGenSetup);
		const threshold = vi.fn();
		threshold.mockReturnValueOnce(2);
		const keyGenClient = {
			setupGroup,
			setupKeyGen,
			threshold,
			participants: vi.fn().mockReturnValueOnce(participants),
		} as unknown as KeyGenClient;
		const consensus = vi.fn();
		consensus.mockReturnValueOnce(ethAddress);
		const protocol = { consensus } as unknown as SafenetProtocol;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: EVENT.gid,
				nextEpoch: 10n,
				deadline: 30n,
				sharesFrom: [],
				complaints: {
					[entryPoint07Address]: { unresponded: 0, total: 1 },
				},
			},
			signing: {},
		};

		const diff = await handleComplaintSubmitted(MACHINE_CONFIG, protocol, keyGenClient, machineStates, EVENT);

		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 3,
				threshold: 2,
				context: calcGroupContext(ethAddress, 10n),
				commitments: keyGenSetup.commitments,
				encryptionPublicKey: keyGenSetup.encryptionPublicKey,
				pok: keyGenSetup.pok,
				poap: keyGenSetup.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x5afe02",
			nextEpoch: 10n,
			deadline: 31n,
		});
		expect(diff.consensus).toStrictEqual({
			epochGroup: [10n, "0x5afe02"],
		});
		expect(consensus).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(
			[participants[0], participants[2], participants[3]],
			2,
			calcGroupContext(ethAddress, 10n),
		);
	});
});
