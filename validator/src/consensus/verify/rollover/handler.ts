import type { Hex } from "viem";
import type { PacketHandler, Typed } from "../engine.js";
import { epochRolloverHash } from "./hashing.js";
import { EpochRolloverPacket, epochRolloverPacketSchema } from "./schemas.js";

export class EpochRolloverHandler implements PacketHandler<EpochRolloverPacket> {
	async hashAndVerify(uncheckedPacket: EpochRolloverPacket): Promise<Hex> {
		const packet = epochRolloverPacketSchema.parse(uncheckedPacket);
		// TODO: verify epoch
		return epochRolloverHash(packet);
	}
}
