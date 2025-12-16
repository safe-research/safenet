import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { KeyGenComplaintSubmittedEvent } from "../transitions/types.js";
import type { MachineStates } from "../types.js";
import { handleComplaintSubmitted } from "./complaintSubmitted.js";

// --- Test Data ---
const EVENT: KeyGenComplaintSubmittedEvent = {
	id: "event_key_gen_complaint_submitted",
	block: 21n,
	index: 0,
	gid: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
	plaintiff: 1n,
	accused: 2n,
};

// --- Tests ---
describe("complaint submitted", () => {
	it("should not handle event if in unexpected state", async () => {
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
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle complaint if unexpected group id", async () => {
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
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle complaint in collecting confirmations if complaint deadline has passed", async () => {
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
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should accept complaints when collecting shares", async () => {
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				complaints: {},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
		});
	});

	it("should accept complaints when collecting confirmations", async () => {
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
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
				complaints: {},
				confirmationsFrom: [],
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
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
				confirmationsFrom: [],
			},
		});
	});

	it("should accept multiple complaints for different accused", async () => {
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				complaints: {
					"1": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				complaints: {
					"1": { unresponded: 1n, total: 1n },
					"2": { unresponded: 1n, total: 1n },
				},
			},
		});
	});

	it("should accept multiple complaints for same accused", async () => {
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(1n);
		const keyGenClient = {
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				complaints: {
					"2": { unresponded: 2n, total: 2n },
				},
			},
		});
	});

	it("should immediately react to complaint when accused", async () => {
		const participantId = vi.fn();
		participantId.mockReturnValueOnce(2n);
		const secretShare = 0x5afe5afe5afen;
		const createSecretShare = vi.fn();
		createSecretShare.mockReturnValueOnce(secretShare);
		const keyGenClient = {
			createSecretShare,
			participantId,
		} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				complaints: {
					"2": { unresponded: 1n, total: 1n },
				},
			},
			signing: {},
		};
		const diff = await handleComplaintSubmitted(keyGenClient, machineStates, EVENT);
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
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
});
