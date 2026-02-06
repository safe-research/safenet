import { zeroAddress } from "viem";
import { describe, expect, it, vi } from "vitest";
import { SafeTransactionHandler, type TransactionCheck } from "./handler.js";
import type { SafeTransactionPacket } from "./schemas.js";

describe("safeTx handler", () => {
	it("should throw on invalid packet", async () => {
		const testCheck = {} as unknown as TransactionCheck;
		const handler = new SafeTransactionHandler(testCheck);
		await expect(
			handler.hashAndVerify({
				type: "invalid packet",
			} as unknown as SafeTransactionPacket),
		).rejects.toThrow();
	});

	it("should throw for invalid operation", async () => {
		const testCheck = {} as unknown as TransactionCheck;
		const handler = new SafeTransactionHandler(testCheck);
		await expect(
			handler.hashAndVerify({
				type: "safe_transaction_packet",
				domain: {
					chain: 23n,
					consensus: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				proposal: {
					epoch: 11n,
					transaction: {
						chainId: 1n,
						account: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
						to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
						value: 0n,
						data: "0xbaddad42",
						operation: 2,
						nonce: 0n,
					},
				},
			} as unknown as SafeTransactionPacket),
		).rejects.toThrow();
	});

	it("should call check for delegatecall tx and return hash", async () => {
		const testCheck = vi.fn();
		const handler = new SafeTransactionHandler(testCheck);
		await expect(
			handler.hashAndVerify({
				type: "safe_transaction_packet",
				domain: {
					chain: 23n,
					consensus: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				proposal: {
					epoch: 11n,
					transaction: {
						chainId: 1n,
						safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
						to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
						value: 0n,
						data: "0xbaddad42",
						operation: 1,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			}),
		).resolves.toBe("0x7e28ff55ec52e4ca345036a233b1442c5e0c6622934a4dee0f7886c6573e8f7f");
		expect(testCheck).toBeCalledTimes(1);
		expect(testCheck).toBeCalledWith({
			chainId: 1n,
			safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
			to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
			value: 0n,
			data: "0xbaddad42",
			operation: 1,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		});
	});

	it("should throw if check throws (delegatecall)", async () => {
		const testCheck = vi.fn();
		testCheck.mockImplementationOnce(() => {
			throw new Error("Invalid");
		});
		const handler = new SafeTransactionHandler(testCheck);
		await expect(
			handler.hashAndVerify({
				type: "safe_transaction_packet",
				domain: {
					chain: 23n,
					consensus: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				proposal: {
					epoch: 11n,
					transaction: {
						chainId: 1n,
						safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
						to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
						value: 0n,
						data: "0xbaddad42",
						operation: 1,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			}),
		).rejects.toStrictEqual(Error("Invalid"));
		expect(testCheck).toBeCalledTimes(1);
		expect(testCheck).toBeCalledWith({
			chainId: 1n,
			safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
			to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
			value: 0n,
			data: "0xbaddad42",
			operation: 1,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		});
	});

	it("should call call check for call tx and return hash", async () => {
		const testCheck = vi.fn();
		const handler = new SafeTransactionHandler(testCheck);
		await expect(
			handler.hashAndVerify({
				type: "safe_transaction_packet",
				domain: {
					chain: 23n,
					consensus: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				proposal: {
					epoch: 11n,
					transaction: {
						chainId: 1n,
						safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
						to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
						value: 0n,
						data: "0xbaddad42",
						operation: 0,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			}),
		).resolves.toBe("0x73d172bcf0980b5240bc471a527626fb5ea145ec0c1c61b1c19f0450f18a2059");
		expect(testCheck).toBeCalledTimes(1);
		expect(testCheck).toBeCalledWith({
			chainId: 1n,
			safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
			to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
			value: 0n,
			data: "0xbaddad42",
			operation: 0,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		});
	});

	it("should throw if check throws (call)", async () => {
		const testCheck = vi.fn();
		testCheck.mockImplementationOnce(() => {
			throw new Error("Invalid");
		});
		const handler = new SafeTransactionHandler(testCheck);
		await expect(
			handler.hashAndVerify({
				type: "safe_transaction_packet",
				domain: {
					chain: 23n,
					consensus: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				proposal: {
					epoch: 11n,
					transaction: {
						chainId: 1n,
						safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
						to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
						value: 0n,
						data: "0xbaddad42",
						operation: 0,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			}),
		).rejects.toStrictEqual(Error("Invalid"));
		expect(testCheck).toBeCalledTimes(1);
		expect(testCheck).toBeCalledWith({
			chainId: 1n,
			safe: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
			to: "0x22Cb221caE98D6097082C80158B1472C45FEd729",
			value: 0n,
			data: "0xbaddad42",
			operation: 0,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		});
	});
});
