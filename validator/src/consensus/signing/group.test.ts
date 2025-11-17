import { keccak256, stringToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { toPoint } from "../../frost/math.js";
import { groupChallenge } from "./group.js";

// --- Tests ---
describe("group", () => {
	it("should generate correct challenge", async () => {
		const groupCommitment = toPoint({
			x: 0x8a3802114b5b6369ae8ba7822bdb029dee0d53fc416225d9198959b83f73215bn,
			y: 0x3020f80cae8f515d58686d5c6e4f1d027a1671348b6402f4e43ce525bda00fbcn,
		});
		const groupPublicKey = toPoint({
			x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75n,
			y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5n,
		});
		const message = keccak256(stringToBytes("hello"));
		expect(groupChallenge(groupCommitment, groupPublicKey, message)).toBe(
			0x092370ad82e7356eb5fe89e9be058a335705b482eaa9832fb81eddd3723647b4n,
		);
	});
});
