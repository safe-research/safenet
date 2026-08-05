import type { Address } from "viem";
import { describe, expect, it } from "vitest";
import { createStatusMapInfo } from "./info";

const ADDR_A = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" as Address;
const ADDR_B = "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB" as Address;

const validatorInfoMap = new Map([
	[ADDR_A, { address: ADDR_A, label: "Alice" }],
	[ADDR_B, { address: ADDR_B, label: "Bob" }],
]);

describe("createStatusMapInfo", () => {
	it("uses correct suffixes when not completed", () => {
		const mapInfo = createStatusMapInfo(validatorInfoMap, false);
		expect(mapInfo(ADDR_A, true)).toBe("Alice ✅");
		expect(mapInfo(ADDR_B, false)).toBe("Bob ⏳");
	});

	it("uses correct suffixes when completed", () => {
		const mapInfo = createStatusMapInfo(validatorInfoMap, true);
		expect(mapInfo(ADDR_A, true)).toBe("Alice ✅");
		expect(mapInfo(ADDR_B, false)).toBe("Bob ❌");
	});
});
