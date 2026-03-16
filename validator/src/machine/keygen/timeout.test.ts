import { ethAddress, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import { makeGroupSetup, makeKeyGenSetup } from "../../__tests__/data/machine.js";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { MachineConfig, MachineStates, RolloverState } from "../types.js";
import { checkKeyGenTimeouts } from "./timeouts.js";

// --- Test Data ---
const MACHINE_CONFIG: MachineConfig = {
	account: ethAddress,
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
				confirmationsFrom: [entryPoint06Address, entryPoint08Address, ethAddress],
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
					[entryPoint07Address]: { unresponded: 1, total: 1 },
				},
				missingSharesFrom: [],
				confirmationsFrom: [entryPoint06Address, entryPoint07Address, ethAddress],
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
			const groupSetup = makeGroupSetup();
			const keyGenSetup = makeKeyGenSetup();
			const consensus = vi.fn();
			consensus.mockReturnValueOnce(ethAddress);
			const protocol = {
				consensus,
			} as unknown as SafenetProtocol;
			const missingCommitments = vi.fn();
			missingCommitments.mockReturnValueOnce([entryPoint07Address]);
			const missingSecretShares = vi.fn();
			missingSecretShares.mockReturnValueOnce([entryPoint07Address]);
			const setupGroup = vi.fn();
			setupGroup.mockReturnValueOnce(groupSetup);
			const setupKeyGen = vi.fn();
			setupKeyGen.mockReturnValueOnce(keyGenSetup);
			const participants = vi.fn();
			participants.mockReturnValueOnce([entryPoint06Address, entryPoint07Address, entryPoint08Address, ethAddress]);
			const keyGenClient = {
				participants,
				setupGroup,
				setupKeyGen,
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
				deadline: 30n,
			});
			expect(diff.consensus).toStrictEqual({
				epochGroup: [10n, "0x5afe02"],
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
				[entryPoint06Address, entryPoint08Address, ethAddress],
				2,
				"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
			);
		});
	});
});
