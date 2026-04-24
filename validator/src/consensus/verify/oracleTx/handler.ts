import { type Address, type Hex, isAddressEqual } from "viem";
import type { Metrics } from "../../../utils/metrics.js";
import type { PacketHandler } from "../engine.js";
import { oracleTxPacketHash } from "./hashing.js";
import { type OracleTransactionPacket, oracleTransactionPacketSchema } from "./schemas.js";

export class OracleTransactionHandler implements PacketHandler<OracleTransactionPacket> {
	constructor(
		private allowedOracles: readonly Address[],
		private metrics?: Pick<Metrics, "transactionChecks">,
	) {}

	async hashAndVerify(uncheckedPacket: OracleTransactionPacket): Promise<Hex> {
		const packet = oracleTransactionPacketSchema.parse(uncheckedPacket);
		if (!this.allowedOracles.some((o) => isAddressEqual(o, packet.proposal.oracle))) {
			this.metrics?.transactionChecks.labels({ result: "oracle_not_allowed" }).inc();
			throw new Error(`Oracle ${packet.proposal.oracle} is not in the allowlist`);
		}
		this.metrics?.transactionChecks.labels({ result: "success" }).inc();
		return oracleTxPacketHash(packet);
	}
}
