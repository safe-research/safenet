import type { Hex } from "viem";
import type { PacketHandler } from "../engine.js";
import { safeTxPacketHash } from "./hashing.js";
import { type MetaTransaction, type SafeTransactionPacket, safeTransactionPacketSchema } from "./schemas.js";

export interface TransactionCheck {
	check(tx: MetaTransaction): void;
}

export class SafeTransactionHandler implements PacketHandler<SafeTransactionPacket> {
	constructor(
		private delegateCallCheck: TransactionCheck,
		private callCheck: TransactionCheck,
	) {}
	async hashAndVerify(uncheckedPacket: SafeTransactionPacket): Promise<Hex> {
		const packet = safeTransactionPacketSchema.parse(uncheckedPacket);
		switch (packet.proposal.transaction.operation) {
			case 0:
				this.callCheck.check(packet.proposal.transaction);
				break;
			case 1:
				this.delegateCallCheck.check(packet.proposal.transaction);
				break;
			default:
				throw new Error("Unknown operation");
		}
		return safeTxPacketHash(packet);
	}
}
