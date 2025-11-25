import type { Hex } from "viem";
import type { PacketHandler } from "../engine.js";
import { safeTxPacketHash } from "./hashing.js";
import {
	type SafeTransactionPacket,
	safeTransactionPacketSchema,
} from "./schemas.js";

export class SafeTransactionHandler
	implements PacketHandler<SafeTransactionPacket>
{
	async hashAndVerify(uncheckedPacket: SafeTransactionPacket): Promise<Hex> {
		const packet = safeTransactionPacketSchema.parse(uncheckedPacket);
		if (packet.proposal.transaction.operation !== 0)
			throw Error("Delegatecall not allowed");
		return safeTxPacketHash(packet);
	}
}
