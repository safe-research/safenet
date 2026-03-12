import { ethAddress, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import { makeGroupSetup } from "../../__tests__/data/machine.js";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { MachineConfig, MachineStates, RolloverState } from "../types.js";
import { checkKeyGenTimeouts } from "./timeouts.js";

// --- Test Data ---
const MACHINE_CONFIG: MachineConfig = {
	participantsInfo: [
		{
			id: 1n,
			address: entryPoint06Address,
			activeFrom: 0n,
		},
		{
			id: 3n,
			address: entryPoint07Address,
			activeFrom: 0n,
		},
		{
			id: 7n,
			address: entryPoint08Address,
			activeFrom: 0n,
		},
		{
			id: 11n,
			address: ethAddress,
			activeFrom: 0n,
		},
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 10n,
};

// --- Tests ---
describe("key gen timeouts", () => {
	it("should not timeout in waiting for genesis", () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: { id: "waiting_for_genesis" },
			signing: {},
		};
		const diff = checkKeyGenTimeouts(MACHINE_CONFIG, protocol, keyGenClient, machineStates, 10n);

		expect(diff).toStrictEqual({});
	});

	it("should not timeout in signing rollover (is handle in signing flow)", () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "sign_rollover",
				groupId: "0x5afe01",
				nextEpoch: 1n,
				message: "0x5afe5afe5afe",
			},
			signing: {},
		};
		const diff = checkKeyGenTimeouts(MACHINE_CONFIG, protocol, keyGenClient, machineStates, 10n);

		expect(diff).toStrictEqual({});
	});

	describe.each([
		{
			description: "collecting commitments",
			rollover: {
				id: "collecting_commitments",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				deadline: 22n,
			} as RolloverState,
			keyGenInvocations: [1, 0],
		},
		{
			description: "collecting shares",
			rollover: {
				id: "collecting_shares",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				deadline: 22n,
				missingSharesFrom: [],
				complaints: {},
			} as RolloverState,
			keyGenInvocations: [0, 1],
		},
		{
			description: "collecting confirmations",
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				complaintDeadline: 12n,
				responseDeadline: 17n,
				deadline: 22n,
				complaints: {},
				missingSharesFrom: [],
				confirmationsFrom: [1n, 7n, 11n],
			} as RolloverState,
			keyGenInvocations: [0, 0],
		},
		{
			description: "waiting for responses",
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				complaintDeadline: 15n,
				responseDeadline: 25n,
				deadline: 35n,
				complaints: {
					"3": { unresponded: 1n, total: 1n },
				},
				missingSharesFrom: [],
				confirmationsFrom: [1n, 3n, 11n],
			} as RolloverState,
			keyGenInvocations: [0, 0],
		},
	])("when $description", ({ rollover, keyGenInvocations }) => {
		it("should not timeout when deadline has not passed", () => {
			const protocol = {} as unknown as SafenetProtocol;
			const keyGenClient = {} as unknown as KeyGenClient;
			const machineStates: MachineStates = {
				rollover,
				signing: {},
			};
			const diff = checkKeyGenTimeouts(MACHINE_CONFIG, protocol, keyGenClient, machineStates, 10n);

			expect(diff).toStrictEqual({});
		});
		it("should trigger key gen after deadline has passed", () => {
			const groupSetup = makeGroupSetup(7n);
			const consensus = vi.fn();
			consensus.mockReturnValueOnce(ethAddress);
			const protocol = {
				consensus,
			} as unknown as SafenetProtocol;
			const missingCommitments = vi.fn();
			missingCommitments.mockReturnValueOnce([3n]);
			const missingSecretShares = vi.fn();
			missingSecretShares.mockReturnValueOnce([3n]);
			const setupGroup = vi.fn();
			setupGroup.mockReturnValueOnce(groupSetup);
			const participants = vi.fn();
			participants.mockReturnValueOnce(MACHINE_CONFIG.participantsInfo);
			const keyGenClient = {
				participants,
				setupGroup,
				missingCommitments,
				missingSecretShares,
			} as unknown as KeyGenClient;
			const machineStates: MachineStates = {
				rollover,
				signing: {},
			};
			const diff = checkKeyGenTimeouts(MACHINE_CONFIG, protocol, keyGenClient, machineStates, 30n);
			expect(diff.actions).toStrictEqual([
				{
					id: "key_gen_start",
					participants: groupSetup.participantsRoot,
					count: 3,
					threshold: 2,
					context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
					participantId: 7n,
					commitments: groupSetup.commitments,
					encryptionPublicKey: groupSetup.encryptionPublicKey,
					pok: groupSetup.pok,
					poap: groupSetup.poap,
				},
			]);
			expect(diff.rollover).toStrictEqual({
				id: "collecting_commitments",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				deadline: 30n,
			});
			expect(diff.consensus).toStrictEqual({
				epochGroup: [10n, { groupId: "0x5afe02", participantId: 7n }],
			});
			expect(diff.signing).toBeUndefined();

			expect(consensus).toBeCalledTimes(1);
			expect(missingCommitments).toBeCalledTimes(keyGenInvocations[0]);
			if (keyGenInvocations[0] > 0) {
				expect(missingCommitments).toBeCalledWith("0x5afe02");
			}
			expect(missingSecretShares).toBeCalledTimes(keyGenInvocations[1]);
			if (keyGenInvocations[1] > 0) {
				expect(missingSecretShares).toBeCalledWith("0x5afe02");
			}
			expect(setupGroup).toBeCalledTimes(1);
			expect(setupGroup).toBeCalledWith(
				[MACHINE_CONFIG.participantsInfo[0], MACHINE_CONFIG.participantsInfo[2], MACHINE_CONFIG.participantsInfo[3]],
				2,
				"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
			);
		});
	});
});
