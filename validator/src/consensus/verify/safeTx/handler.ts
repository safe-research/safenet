import type { Hex } from "viem";
import type { Metrics } from "../../../utils/metrics.js";
import type { PacketHandler } from "../engine.js";
import { TransactionCheckError } from "./checks/errors.js";
import { safeTxPacketHash } from "./hashing.js";
import { type SafeTransaction, type SafeTransactionPacket, safeTransactionPacketSchema } from "./schemas.js";

export type TransactionCheck = (tx: SafeTransaction) => void;

export class SafeTransactionHandler implements PacketHandler<SafeTransactionPacket> {
	constructor(
		private check: TransactionCheck,
		private metrics?: Pick<Metrics, "transactionChecks">,
	) {}
	async hashAndVerify(uncheckedPacket: SafeTransactionPacket): Promise<Hex> {
		const packet = safeTransactionPacketSchema.parse(uncheckedPacket);
		try {
			this.check(packet.proposal.transaction);
			this.metrics?.transactionChecks.labels({ result: "success" }).inc();
		} catch (error) {
			const label = error instanceof TransactionCheckError ? error.code : "unknown";
			this.metrics?.transactionChecks.labels({ result: label }).inc();
			throw error;
		}
		return safeTxPacketHash(packet);
	}
}
