import { describe, expect, it } from "vitest";
import { calcGroupId } from "./utils.js";

const participantsRoot = "0x0000000000000000000000000000000000000000000000000000000000000001" as `0x${string}`;
const context = "0x0000000000000000000000000000000000000000000000000000000000000002" as `0x${string}`;

describe("calcGroupId", () => {
	it("last 8 bytes are zero", () => {
		const id = calcGroupId(participantsRoot, 3, 2, context);
		expect(id.slice(-16)).toBe("0000000000000000");
	});
});
