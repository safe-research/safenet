import type { Address, Hex } from "viem";
import { describe, expect, it } from "vitest";
import { safeWalletSafeUrl, safeWalletTxUrl } from "./wallet";

const SAFE = "0xA1b2C3d4E5F6a1B2c3D4e5f6A1b2C3d4E5f6A1B2" as Address;
const SAFE_TX_HASH = "0x9f1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab7a" as Hex;

describe("safeWalletTxUrl", () => {
	it("returns the correct URL format", () => {
		const url = safeWalletTxUrl("eth", SAFE, SAFE_TX_HASH);
		expect(url).toBe(`https://app.safe.global/transactions/tx?safe=eth:${SAFE}&id=${SAFE_TX_HASH}`);
	});

	it("uses the provided shortName and safe address", () => {
		const url = safeWalletTxUrl("base", SAFE, SAFE_TX_HASH);
		expect(url).toContain("safe=base:");
		expect(url).toContain(SAFE);
	});
});

describe("safeWalletSafeUrl", () => {
	it("returns the correct URL format", () => {
		const url = safeWalletSafeUrl("base", SAFE);
		expect(url).toBe(`https://app.safe.global/balances?safe=base:${SAFE}`);
	});

	it("uses the provided shortName and safe address", () => {
		const url = safeWalletSafeUrl("gno", SAFE);
		expect(url).toContain("safe=gno:");
		expect(url).toContain(SAFE);
	});
});
