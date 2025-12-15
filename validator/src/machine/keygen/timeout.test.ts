import { ethAddress, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { MachineConfig, MachineStates, RolloverState } from "../types.js";
import { checkKeyGenTimeouts } from "./timeouts.js";

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
			id: 7n,
			address: entryPoint08Address,
		},
		{
			id: 11n,
			address: entryPoint08Address,
		},
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 10n,
};

// --- Tests ---
describe("key gen timeouts", () => {
	it("should not timeout in waiting for rollover", () => {
		const protocol = {} as unknown as ShieldnetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: { id: "waiting_for_rollover" },
			signing: {},
		};
		const diff = checkKeyGenTimeouts(MACHINE_CONFIG, protocol, keyGenClient, machineStates, 10n);

		expect(diff).toStrictEqual({});
	});

	it("should not timeout in signing rollover (is handle in signing flow)", () => {
		const protocol = {} as unknown as ShieldnetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			rollover: {
				id: "sign_rollover",
				groupId: "0x5afe01",
				nextEpoch: 1n,
				message: "0x5afe5afe5afe",
				responsible: 3n,
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
			} as RolloverState,
			keyGenInvocations: [0, 1],
		},
		{
			description: "collecting confirmations",
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				deadline: 22n,
				sharesFrom: [1n, 3n],
			} as RolloverState,
			keyGenInvocations: [0, 0],
		},
	])("when in $description", ({ rollover, keyGenInvocations }) => {
		it("should not timeout when deadline has not passed", () => {
			const protocol = {} as unknown as ShieldnetProtocol;
			const keyGenClient = {} as unknown as KeyGenClient;
			const machineStates: MachineStates = {
				rollover,
				signing: {},
			};
			const diff = checkKeyGenTimeouts(MACHINE_CONFIG, protocol, keyGenClient, machineStates, 10n);

			expect(diff).toStrictEqual({});
		});
		it("should trigger key gen after deadline has passed", ({ skip }) => {
			if (rollover.id === "collecting_confirmations") skip();
			const groupSetup = {
				groupId: "0x5afe02",
				participantsRoot: "0x5afe5afe5afe",
				participantId: 7n,
				commitments: [TEST_POINT],
				pok: {
					r: TEST_POINT,
					mu: 100n,
				},
				poap: ["0x5afe5afe5afe01"],
			};
			const consensus = vi.fn();
			consensus.mockReturnValueOnce(ethAddress);
			const protocol = {
				consensus,
			} as unknown as ShieldnetProtocol;
			const missingCommitments = vi.fn();
			missingCommitments.mockReturnValueOnce([3n]);
			const missingSecretShares = vi.fn();
			missingSecretShares.mockReturnValueOnce([3n]);
			const setupGroup = vi.fn();
			setupGroup.mockReturnValueOnce(groupSetup);
			const keyGenClient = {
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
					count: 3n,
					threshold: 2n,
					context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
					participantId: 7n,
					commitments: groupSetup.commitments,
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
				[
					MACHINE_CONFIG.defaultParticipants[0],
					MACHINE_CONFIG.defaultParticipants[2],
					MACHINE_CONFIG.defaultParticipants[3],
				],
				3n,
				2n,
				"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000a",
			);
		});
	});
});
