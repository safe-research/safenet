import type { Address } from "viem";
import type { ParticipantInfo } from "../types/interfaces.js";

export const participantsForEpoch = (participants: ParticipantInfo[], epoch: bigint): Address[] => {
	return Array.from(
		new Set(
			participants
				.filter((p) => p.activeFrom <= epoch && (p.activeBefore === undefined || epoch < p.activeBefore))
				.map((p) => p.address),
		),
	).sort((a, b) => Number(BigInt(a) - BigInt(b)));
};
