import { describe, expect, it } from "vitest";
import { shortAddress } from "./address";

describe("address", () => {
	const LOWER = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
	const SHORT = "0xDeaD…beeF";

	it("returns checksummed address", () => {
		expect(shortAddress(LOWER)).toBe(SHORT);
	});
});
