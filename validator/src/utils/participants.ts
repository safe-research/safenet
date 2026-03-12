import type { Participant } from "../consensus/storage/types.js";
import type { ParticipantInfo } from "../types/interfaces.js";

export const participantsForEpoch = (participants: ParticipantInfo[], epoch: bigint): Participant[] => {
	const participantMap = new Map<string, Participant>();
	for (const participant of participants) {
		if (
			participant.activeFrom <= epoch &&
			(participant.activeBefore === undefined || epoch < participant.activeBefore)
		) {
			participantMap.set(participant.id.toString(), { address: participant.address, id: participant.id });
		}
	}
	return Array.from(participantMap.values()).sort((a, b) => (a.id < b.id ? -1 : 1));
};
