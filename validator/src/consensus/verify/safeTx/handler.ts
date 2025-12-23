import type { Hex } from "viem";
import type { PacketHandler } from "../engine.js";
import { safeTxPacketHash } from "./hashing.js";
import { type MetaTransaction, type SafeTransactionPacket, safeTransactionPacketSchema } from "./schemas.js";

export interface TransactionCheck {
	check(tx: MetaTransaction): void;
}

export class SafeTransactionHandler implements PacketHandler<SafeTransactionPacket> {
	constructor(
		private check: TransactionCheck,
	) {}
	async hashAndVerify(uncheckedPacket: SafeTransactionPacket): Promise<Hex> {
		const packet = safeTransactionPacketSchema.parse(uncheckedPacket);
		this.check.check(packet.proposal.transaction);
		return safeTxPacketHash(packet);
	}
}
