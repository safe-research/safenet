import { ethAddress, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import { makeGroupSetup } from "../../__tests__/data/machine.js";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { KeyGenComplaintResponsedEvent as KeyGenComplaintRespondedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates } from "../types.js";
import { handleComplaintResponded } from "./complaintResponse.js";

// --- Test Data ---
const MACHINE_CONFIG: MachineConfig = {
	participantsInfo: [
		{
			address: entryPoint06Address,
			activeFrom: 0n,
		},
		{
			address: entryPoint07Address,
			activeFrom: 0n,
		},
		{
			address: entryPoint08Address,
			activeFrom: 0n,
		},
		{
			address: ethAddress,
			activeFrom: 0n,
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
	plaintiff: entryPoint06Address,
	accused: entryPoint08Address,
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
		const participant = vi.fn();
		participant.mockReturnValueOnce(entryPoint06Address);
		const verifySecretShare = vi.fn();
		verifySecretShare.mockReturnValueOnce(true);
		const keyGenClient = {
			verifySecretShare,
			participant,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					[entryPoint08Address]: { total: 1, unresponded: 1 },
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
					[entryPoint08Address]: { unresponded: 0, total: 1 },
				},
			},
			actions: [],
		});
	});

	it("should accept complaints when collecting confirmations", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const participant = vi.fn();
		participant.mockReturnValueOnce(entryPoint06Address);
		const verifySecretShare = vi.fn();
		verifySecretShare.mockReturnValueOnce(true);
		const keyGenClient = {
			verifySecretShare,
			participant,
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
					[entryPoint08Address]: { total: 1, unresponded: 1 },
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
					[entryPoint08Address]: { unresponded: 0, total: 1 },
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
		const groupSetup = makeGroupSetup();
		const setupGroup = vi.fn();
		setupGroup.mockReturnValueOnce(groupSetup);
		const participant = vi.fn();
		participant.mockReturnValueOnce(entryPoint07Address);
		const verifySecretShare = vi.fn();
		verifySecretShare.mockReturnValueOnce(false);
		const participants = vi.fn();
		participants.mockReturnValueOnce(MACHINE_CONFIG.participantsInfo.map((p) => p.address));
		const keyGenClient = {
			participants,
			setupGroup,
			verifySecretShare,
			participant,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {
					[entryPoint08Address]: { unresponded: 1, total: 1 },
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
				epochGroup: [10n, "0x5afe02"],
			},
			actions: [
				{
					id: "key_gen_start",
					participants: groupSetup.participantsRoot,
					count: 3,
					threshold: 2,
					context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
					commitments: groupSetup.commitments,
					encryptionPublicKey: groupSetup.encryptionPublicKey,
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
		const groupSetup = makeGroupSetup();
		const setupGroup = vi.fn();
		setupGroup.mockReturnValueOnce(groupSetup);
		const participant = vi.fn();
		participant.mockReturnValueOnce(entryPoint06Address);
		const participants = vi.fn();
		participants.mockReturnValueOnce(MACHINE_CONFIG.participantsInfo.map((p) => p.address));
		const registerPlainKeyGenSecret = vi.fn();
		registerPlainKeyGenSecret.mockReturnValueOnce("invalid_share");
		const keyGenClient = {
			participants,
			setupGroup,
			registerPlainKeyGenSecret,
			participant,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [entryPoint08Address],
				complaints: {
					[entryPoint08Address]: { unresponded: 1, total: 1 },
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
				epochGroup: [10n, "0x5afe02"],
			},
			actions: [
				{
					id: "key_gen_start",
					participants: groupSetup.participantsRoot,
					count: 3,
					threshold: 2,
					context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
					commitments: groupSetup.commitments,
					encryptionPublicKey: groupSetup.encryptionPublicKey,
					pok: groupSetup.pok,
					poap: groupSetup.poap,
				},
			],
		});
	});

	it("should remove missing share once received", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const participant = vi.fn();
		participant.mockReturnValueOnce(entryPoint06Address);
		const registerPlainKeyGenSecret = vi.fn();
		registerPlainKeyGenSecret.mockReturnValueOnce("pending_shares");
		const keyGenClient = {
			registerPlainKeyGenSecret,
			participant,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				missingSharesFrom: [entryPoint08Address],
				confirmationsFrom: [],
				complaints: {
					[entryPoint08Address]: { unresponded: 1, total: 1 },
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
					[entryPoint08Address]: { unresponded: 0, total: 1 },
				},
			},
			actions: [],
		});
	});

	it("should trigger confirmation if missing share in collecting confirmations", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const participant = vi.fn();
		participant.mockReturnValueOnce(entryPoint06Address);
		const registerPlainKeyGenSecret = vi.fn();
		registerPlainKeyGenSecret.mockReturnValueOnce("shares_completed");
		const keyGenClient = {
			registerPlainKeyGenSecret,
			participant,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 25n,
				responseDeadline: 30n,
				deadline: 30n,
				missingSharesFrom: [entryPoint08Address],
				confirmationsFrom: [],
				complaints: {
					[entryPoint08Address]: { unresponded: 1, total: 1 },
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
					[entryPoint08Address]: { unresponded: 0, total: 1 },
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
