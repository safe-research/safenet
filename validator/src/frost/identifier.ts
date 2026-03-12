import { type Address, hexToBytes } from "viem";
import { hid } from "./hashes.js";

export const deriveParticipantId = (participant: Address): bigint => {
	return hid(hexToBytes(participant));
};

export const sortedParticipantIds = (participants: readonly Address[]): bigint[] => {
	return participants.map(deriveParticipantId).sort((a, b) => Number(a - b));
};

export const toParticipantIdMap = <T>(map: Map<Address, T>): Map<bigint, T> => {
	return new Map(map.entries().map(([participant, commitment]) => [deriveParticipantId(participant), commitment]));
};
