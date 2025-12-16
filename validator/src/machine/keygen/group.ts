import { type Address, encodePacked, type Hex, keccak256, zeroHash } from "viem";
import { calcGroupId } from "../../consensus/keyGen/utils.js";
import { calculateParticipantsRoot } from "../../consensus/merkle.js";
import type { GroupId } from "../../frost/types.js";
import type { MachineConfig } from "../types.js";

export type GroupParameters = {
	count: bigint;
	threshold: bigint;
};

export const calcGroupParameters = (participantCount: number): GroupParameters => {
	const count = BigInt(participantCount);
	const threshold = count / 2n + 1n;
	// TODO: Handle cases where the group size is too small.
	return { count, threshold };
};

export const calcGroupContext = (consensus: Address, epoch: bigint): Hex => {
	// 4 bytes version, 20 bytes address, 8 bytes epoch number
	return encodePacked(["uint32", "address", "uint64"], [0, consensus, epoch]);
};

export type GenesisGroup = {
	id: GroupId;
	participantsRoot: Hex;
	count: bigint;
	threshold: bigint;
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
