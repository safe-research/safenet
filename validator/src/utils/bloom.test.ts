import type { Hex } from "viem";
import { describe, expect, it } from "vitest";
import { isInBloom } from "./bloom.js";

const ZERO = `0x${"00".repeat(256)}` as const;

describe("bloom", () => {
	// Test data taken from Alloy:
	// <https://github.com/alloy-rs/core/blob/6b3ffe33e7ebd828f8103b850e1f815868f2e8dd/crates/primitives/src/bits/bloom.rs#L237>
	it("should check inclusion", async () => {
		const bloom =
			"0x00000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002020000000000000000000000000000000000000000000008000000001000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
		const address = "0xeF2d6D194084c2de36E0dABfcE45D046B37D1106";
		const topic = "0x02c69be41d0b7e40352fc85be1cd65eb03d40ef8427a0ca4596b1ead9a00e9fc";

		expect(isInBloom(ZERO, address)).toBe(false);
		expect(isInBloom(ZERO, topic)).toBe(false);
		expect(isInBloom(bloom, address)).toBe(true);
		expect(isInBloom(bloom, topic)).toBe(true);

		// Note that if we flip one bit back to 0, then only exactly one of the fields will be
		// included in the bloom filter.
		const flipped = bloom.replace("1", "0") as Hex;
		expect(isInBloom(flipped, address) !== isInBloom(flipped, topic)).toBe(true);
	});
});
