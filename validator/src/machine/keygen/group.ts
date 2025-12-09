import { type Address, encodePacked, type Hex, zeroAddress } from "viem";
import { calcGroupId } from "../../consensus/keyGen/utils.js";
import { calculateParticipantsRoot } from "../../consensus/merkle.js";
import type { GroupId } from "../../frost/types.js";
import type { MachineConfig } from "../types.js";

export type GroupParameters = {
	count: bigint;
	threshold: bigint;
	context: Hex;
};

export const calcGroupParameters = (participantCount: number, consensus: Address, epoch: bigint): GroupParameters => {
	const count = BigInt(participantCount);
	const threshold = count / 2n + 1n;
	// 4 bytes version, 20 bytes address, 8 bytes epoch number
	const context = encodePacked(["uint32", "address", "uint64"], [0, consensus, epoch]);
	// TODO: Handle cases where the group size is too small.
	return { count, threshold, context };
};

export const calcGenesisGroupId = ({ defaultParticipants }: Pick<MachineConfig, "defaultParticipants">): GroupId => {
	const participantsRoot = calculateParticipantsRoot(defaultParticipants);
	const { count, threshold, context } = calcGroupParameters(defaultParticipants.length, zeroAddress, 0n);
	return calcGroupId(participantsRoot, count, threshold, context);
};
