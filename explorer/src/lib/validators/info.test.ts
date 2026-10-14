import type { Address } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import { activeValidators, createStatusMapInfo, loadValidatorInfoMap } from "./info";

const ADDR_A = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" as Address;
const ADDR_B = "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB" as Address;

const validatorInfoMap = new Map([
	[ADDR_A, { address: ADDR_A, label: "Alice", isActive: true }],
	[ADDR_B, { address: ADDR_B, label: "Bob", isActive: false }],
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

describe("activeValidators", () => {
	it("only returns validators marked as active", () => {
		expect(activeValidators(validatorInfoMap)).toEqual([ADDR_A]);
	});

	it("returns no validators without info", () => {
		expect(activeValidators(null)).toEqual([]);
	});
});

describe("loadValidatorInfoMap", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it("reads whether validators are active, defaulting to active", async () => {
		const active = "0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe";
		const inactive = "0xb0E735D4a3b70195420E0ae933689A55750CFcd2";
		vi.stubGlobal(
			"fetch",
			vi.fn(async () =>
				Response.json([
					{ address: active, label: "Alice" },
					{ address: inactive, label: "Bob", is_active: false },
				]),
			),
		);
		const map = await loadValidatorInfoMap("https://example.com/validator-info.json");
		expect(map.get(active)?.isActive).toBe(true);
		expect(map.get(inactive)?.isActive).toBe(false);
	});
});
