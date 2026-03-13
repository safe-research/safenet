import { type Address, hexToBytes } from "viem";
import { hid } from "./hashes.js";

export const deriveParticipantId = (participant: Address): bigint => {
	return hid(hexToBytes(participant));
};

export const sortedParticipants = (participants: readonly Address[]): { id: bigint; address: Address }[] => {
	return participants
		.map((address) => ({ id: deriveParticipantId(address), address }))
		.sort((a, b) => Number(a.id - b.id));
};

export const toParticipantIdMap = <T>(map: Map<Address, T>): Map<bigint, T> => {
	return new Map(map.entries().map(([participant, commitment]) => [deriveParticipantId(participant), commitment]));
};
