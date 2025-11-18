import { describe, expect, it } from "vitest";
import { SafeTransactionHandler } from "./handler.js";
import type { SafeTransactionPacket } from "./schemas.js";

describe("safeTx handler", () => {
	it("should throw on invalid packet", async () => {
		const handler = new SafeTransactionHandler();
		await expect(
			handler.hashAndVerify({
				type: "invalid packet",
			} as unknown as SafeTransactionPacket),
		).rejects.toThrow();
	});

	it("should throw for delegatecall tx", async () => {
		const handler = new SafeTransactionHandler();
		await expect(
			handler.hashAndVerify({
				type: "safe_transaction_packet",
				domain: {
					chain: 23n,
					safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				transaction: {
					to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
					value: 0n,
					data: "0xbaddad42",
					operation: 1,
					nonce: 0n,
				},
			}),
		).rejects.toStrictEqual(Error("Delegatecall not allowed"));
	});

	it("should return correct hash", async () => {
		const handler = new SafeTransactionHandler();
		await expect(
			handler.hashAndVerify({
				type: "safe_transaction_packet",
				domain: {
					chain: 23n,
					safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				transaction: {
					to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
					value: 0n,
					data: "0xbaddad42",
					operation: 0,
					nonce: 0n,
				},
			}),
		).resolves.toBe(
			"0x1545182417c12df28ad6e413a98a8da6ec3e5d37e2700df76f3f6eb3795352fc",
		);
	});
});
