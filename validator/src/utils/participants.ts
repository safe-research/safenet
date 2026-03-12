import type { Participant } from "../consensus/storage/types.js";
import type { ParticipantInfo } from "../types/interfaces.js";

export const participantsForEpoch = (participants: ParticipantInfo[], epoch: bigint): Participant[] => {
	const participantMap = new Map<string, Participant>();
	for (const participant of participants) {
		if (
			participant.activeFrom <= epoch &&
			(participant.activeUntil === undefined || epoch <= participant.activeUntil)
		) {
			participantMap.set(participant.id.toString(), { address: participant.address, id: participant.id });
		}
	}
	return Array.from(participantMap.values()).sort((a, b) => (a < b ? -1 : 1));
};
