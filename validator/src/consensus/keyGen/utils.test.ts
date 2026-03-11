import { describe, expect, it } from "vitest";
import { calcGroupId } from "./utils.js";

const participantsRoot = "0x0000000000000000000000000000000000000000000000000000000000000001" as `0x${string}`;
const context = "0x0000000000000000000000000000000000000000000000000000000000000002" as `0x${string}`;

describe("calcGroupId", () => {
	it("returns the expected deterministic group ID with last 8 bytes zeroed", () => {
		const id = calcGroupId(participantsRoot, 3, 2, context);
		expect(id).toBe("0x5a646c47d456084e87ea4b1ac6ef069d1079c21f1401c60c0000000000000000");
	});
});
