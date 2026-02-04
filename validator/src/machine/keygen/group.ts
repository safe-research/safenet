import { type Address, encodePacked, type Hex, keccak256, zeroHash } from "viem";
import { calcGroupId } from "../../consensus/keyGen/utils.js";
import { calculateParticipantsRoot } from "../../consensus/merkle.js";
import type { GroupId } from "../../frost/types.js";
import type { MachineConfig } from "../types.js";

export type GroupParameters = {
	count: number;
	threshold: number;
};

/**
 * Note on threshold and minimum participant count:
 * With M as the count of malicious nodes
 * and T as the minimum require signers (based on a percentage threshold t)
 * it always needs to hold that
 * M < T
 * for any sub group
 *
 * With N as the default participants count
 * and D as the number of dropped participants
 * the calculation is:
 * M < t * (N - D)
 *
 * Solving for D, to calculate the miminum participation count:
 * D < N - M / t
 *
 * If BFT (aka M) is 50%, then D is 0, therefore the target BFT is 33%.
 * To achive a BFT of 33%, assume M = (1 / 3) * N
 * for the default participants:
 * D < N - (1 / 3) * N / t
 * D < N - N / (3 * t)
 *
 * With t set to 1 / 2 (see calcGroupParameters):
 * D < N / 3
 *
 * Meaning for M < (1 / 3) * N to hold, less than a third of the participant can drop.
 *
 * Therefore the minimum participant set must be more than 2 / 3 aka 66% of the default participant count
 */

export const calcMinimumParticipants = ({
	defaultParticipants,
}: Pick<MachineConfig, "defaultParticipants">): number => {
	const count = defaultParticipants.length;
	// The defined minimum participantion group size is 2/3 or 66.66...%
	return Math.max(2, Math.floor((count * 2) / 3) + 1);
};

export const calcGroupParameters = (participantCount: number): GroupParameters => {
	const count = participantCount;
	// The defined threshold is 1/2 or 50%
	const threshold = Math.floor(count / 2) + 1;
	return { count, threshold };
};

export const calcGroupContext = (consensus: Address, epoch: bigint): Hex => {
	// 4 bytes version, 20 bytes address, 8 bytes epoch number
	return encodePacked(["uint32", "address", "uint64"], [0, consensus, epoch]);
};

export type GenesisGroup = {
	id: GroupId;
	participantsRoot: Hex;
	count: number;
	threshold: number;
	context: Hex;
};

export const calcGenesisGroup = ({
	defaultParticipants,
	genesisSalt,
}: Pick<MachineConfig, "defaultParticipants" | "genesisSalt">): GenesisGroup => {
	const participantsRoot = calculateParticipantsRoot(defaultParticipants);
	const { count, threshold } = calcGroupParameters(defaultParticipants.length);
	// For genesis, we don't know the consensus contract address since it
	// depends on the genesis group ID (🐓 and 🥚 problem). Instead, compute a
	// different context based on the user-provided genesis salt (allowing the
	// genesis group ID to be parameterized and the same validator set to work
	// for multiple consensus contracts without needing to rotate the validator
	// accounts).
	const context =
		genesisSalt === zeroHash ? zeroHash : keccak256(encodePacked(["string", "bytes32"], ["genesis", genesisSalt]));
	return {
		id: calcGroupId(participantsRoot, count, threshold, context),
		participantsRoot,
		count,
		threshold,
		context,
	};
};
