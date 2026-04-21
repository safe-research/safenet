import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import { OracleTransactionHandler } from "./handler.js";
import { oracleTxPacketHash } from "./hashing.js";
import type { OracleTransactionPacket } from "./schemas.js";

const ORACLE_ADDRESS = "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97";

const VALID_PACKET: OracleTransactionPacket = {
	type: "oracle_transaction_packet",
	domain: {
		chain: 23n,
		consensus: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
	},
	proposal: {
		epoch: 11n,
		oracle: ORACLE_ADDRESS,
		transaction: {
			chainId: 1n,
			safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
			to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
			value: 0n,
			data: "0x",
			operation: 0,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		},
	},
};

describe("oracle transaction handler", () => {
	it("should throw on invalid packet (bad oracle address)", async () => {
		const handler = new OracleTransactionHandler([ORACLE_ADDRESS]);
		await expect(
			handler.hashAndVerify({
				type: "oracle_transaction_packet",
				domain: { chain: 23n, consensus: "0x22Cb221caE98D6097082C80158B1472C45FEd729" },
				proposal: {
					epoch: 11n,
					oracle: "not-an-address",
					transaction: VALID_PACKET.proposal.transaction,
				},
			} as unknown as OracleTransactionPacket),
		).rejects.toThrow();
	});

	it("should throw if oracle is not in the allowlist", async () => {
		const handler = new OracleTransactionHandler([]);
		await expect(handler.hashAndVerify(VALID_PACKET)).rejects.toThrow(
			`Oracle ${ORACLE_ADDRESS} is not in the allowlist`,
		);
	});

	it("should return correct EIP-712 hash when oracle is in the allowlist", async () => {
		const handler = new OracleTransactionHandler([ORACLE_ADDRESS]);
		const hash = await handler.hashAndVerify(VALID_PACKET);
		expect(hash).toBe(oracleTxPacketHash(VALID_PACKET));
	});
});
