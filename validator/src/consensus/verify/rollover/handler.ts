import type { Hex } from "viem";
import type { PacketHandler } from "../engine.js";
import { epochRolloverHash } from "./hashing.js";
import { type EpochRolloverPacket, epochRolloverPacketSchema } from "./schemas.js";

export type EpochCheck = (rollover: EpochRolloverPacket["rollover"]) => void;

export class EpochRolloverHandler implements PacketHandler<EpochRolloverPacket> {
	constructor(private check?: EpochCheck) {}

	async hashAndVerify(uncheckedPacket: EpochRolloverPacket): Promise<Hex> {
		const packet = epochRolloverPacketSchema.parse(uncheckedPacket);

		if (packet.rollover.proposedEpoch <= packet.rollover.activeEpoch) {
			throw new Error(
				`Invalid epoch rollover: proposedEpoch (${packet.rollover.proposedEpoch}) must be greater than activeEpoch (${packet.rollover.activeEpoch})`,
			);
		}

		this.check?.(packet.rollover);

		return epochRolloverHash(packet);
	}
}
