import type { Address } from "viem";
import type { ParticipantInfo } from "../types/interfaces.js";

export const participantsForEpoch = (participants: ParticipantInfo[], epoch: bigint): Address[] => {
	return [
		...new Set(
			participants
				.filter((p) => p.activeFrom <= epoch && (p.activeBefore === undefined || epoch < p.activeBefore))
				.map((p) => p.address),
		),
	];
};
