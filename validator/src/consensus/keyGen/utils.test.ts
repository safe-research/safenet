import { describe, expect, it } from "vitest";
import { calcGroupId } from "./utils.js";

const participantsRoot = "0x0000000000000000000000000000000000000000000000000000000000000001" as `0x${string}`;
const context = "0x0000000000000000000000000000000000000000000000000000000000000002" as `0x${string}`;

describe("calcGroupId", () => {
	it("deterministic: same inputs → same output", () => {
		const id1 = calcGroupId(participantsRoot, 3, 2, context);
		const id2 = calcGroupId(participantsRoot, 3, 2, context);
		expect(id1).toBe(id2);
	});

	it("last 8 bytes are zero", () => {
		const id = calcGroupId(participantsRoot, 3, 2, context);
		expect(id.slice(-16)).toBe("0000000000000000");
	});

	it("different count → different result", () => {
		const id1 = calcGroupId(participantsRoot, 3, 2, context);
		const id2 = calcGroupId(participantsRoot, 4, 2, context);
		expect(id1).not.toBe(id2);
	});

	it("different threshold → different result", () => {
		const id1 = calcGroupId(participantsRoot, 3, 2, context);
		const id2 = calcGroupId(participantsRoot, 3, 3, context);
		expect(id1).not.toBe(id2);
	});

	it("different participantsRoot → different result", () => {
		const otherRoot = "0x0000000000000000000000000000000000000000000000000000000000000003" as `0x${string}`;
		const id1 = calcGroupId(participantsRoot, 3, 2, context);
		const id2 = calcGroupId(otherRoot, 3, 2, context);
		expect(id1).not.toBe(id2);
	});

	it("different context → different result", () => {
		const otherContext = "0x0000000000000000000000000000000000000000000000000000000000000004" as `0x${string}`;
		const id1 = calcGroupId(participantsRoot, 3, 2, context);
		const id2 = calcGroupId(participantsRoot, 3, 2, otherContext);
		expect(id1).not.toBe(id2);
	});

	it("returns a 0x-prefixed hex string of length 66", () => {
		const id = calcGroupId(participantsRoot, 3, 2, context);
		expect(id).toMatch(/^0x[0-9a-f]{64}$/i);
		expect(id.length).toBe(66);
	});
});
