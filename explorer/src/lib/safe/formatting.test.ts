import type { Hex } from "viem";
import { describe, expect, it } from "vitest";
import type { ChainInfo } from "@/lib/chains";
import { formatBlockAge, formatHashShort } from "./formatting";

const chainWith = (blockTime?: number) => ({ blockTime }) as ChainInfo;

const STANDARD_HASH: Hex = "0x9f123456789a7aBcdeadbeef12345678deadbeef12345678deadbeef12345678";

describe("formatHashShort", () => {
	it("truncates a standard 32-byte hash to first and last 4 bytes", () => {
		expect(formatHashShort(STANDARD_HASH)).toBe("0x9f123456…12345678");
	});

	it("returns the full hash when the hex part is shorter than 16 characters", () => {
		const shortHash = "0xdeadbeef" as Hex;
		expect(formatHashShort(shortHash)).toBe("0xdeadbeef");
	});

	it("handles exactly 16 hex characters without truncation", () => {
		const hash = "0x1234567890abcdef" as Hex;
		expect(formatHashShort(hash)).toBe("0x12345678…90abcdef");
	});
});

describe("formatBlockAge", () => {
	it("formats block diff in the seconds range", () => {
		// 4 blocks × 12 s = 48 s
		expect(formatBlockAge(4n, chainWith(12_000))).toBe("48s ago");
	});

	it("formats block diff just below one minute as seconds", () => {
		// 4 blocks × 12 s = 48 s  (< 60)
		expect(formatBlockAge(4n, chainWith(12_000))).toBe("48s ago");
	});

	it("formats block diff in the minutes range", () => {
		// 100 blocks × 12 s = 1200 s = 20 m
		expect(formatBlockAge(100n, chainWith(12_000))).toBe("20m ago");
	});

	it("formats block diff >= 1 hour as YYYY-MM-DD date", () => {
		// 1000 blocks × 12 s = 12000 s = 200 min (> 1 h)
		// Use a fixed `now` to make the output deterministic
		const now = new Date("2026-03-10T12:00:00Z").getTime();
		const result = formatBlockAge(1000n, chainWith(12_000), now);
		// 12000 s before 2026-03-10T12:00:00Z → 2026-03-10T08:40:00Z → date "2026-03-10"
		expect(result).toMatch(/^\d{4}-\d{2}-\d{2}$/);
		expect(result).toBe("2026-03-10");
	});

	it("uses 12 s fallback when chain.blockTime is undefined", () => {
		// 4 blocks × 12 s fallback = 48 s
		expect(formatBlockAge(4n, chainWith(undefined))).toBe("48s ago");
	});

	it("uses chain-specific block time (e.g. 2 s for Base)", () => {
		// 20 blocks × 2 s = 40 s
		expect(formatBlockAge(20n, chainWith(2_000))).toBe("40s ago");
	});
});
