// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import type { Address } from "viem";
import { afterEach, describe, expect, it } from "vitest";
import { createMapInfo, ValidatorList } from "./ValidatorList";

afterEach(cleanup);

const ADDR_A = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" as Address;
const ADDR_B = "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB" as Address;
const ADDR_C = "0xCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC" as Address;

const mapInfo = createMapInfo(
	new Map([
		[ADDR_A, { address: ADDR_A, label: "Alice" }],
		[ADDR_B, { address: ADDR_B, label: "Bob" }],
		[ADDR_C, { address: ADDR_C, label: "Carol" }],
	]),
);

describe("ValidatorList", () => {
	it("renders active validators with ✅ suffix", () => {
		render(<ValidatorList all={[ADDR_A, ADDR_B]} active={[ADDR_A]} mapInfo={mapInfo} completed={false} />);
		expect(screen.getByText("Alice ✅")).toBeTruthy();
	});

	it("renders inactive validators with ⏳ when not completed", () => {
		render(<ValidatorList all={[ADDR_A, ADDR_B]} active={[ADDR_A]} mapInfo={mapInfo} completed={false} />);
		expect(screen.getByText("Bob ⏳")).toBeTruthy();
	});

	it("renders inactive validators with ❌ when completed", () => {
		render(<ValidatorList all={[ADDR_A, ADDR_B]} active={[ADDR_A]} mapInfo={mapInfo} completed={true} />);
		expect(screen.getByText("Bob ❌")).toBeTruthy();
	});

	it("renders each validator as a span with the address as title", () => {
		render(<ValidatorList all={[ADDR_A, ADDR_B]} active={[ADDR_A, ADDR_B]} mapInfo={mapInfo} completed={false} />);
		const aliceSpan = screen.getByText("Alice ✅");
		const bobSpan = screen.getByText("Bob ✅");
		expect(aliceSpan.getAttribute("title")).toBe(ADDR_A);
		expect(bobSpan.getAttribute("title")).toBe(ADDR_B);
	});

	it("uses the validator address as title when no label is available", () => {
		const noLabelMapInfo = createMapInfo(null);
		render(<ValidatorList all={[ADDR_A]} active={[ADDR_A]} mapInfo={noLabelMapInfo} completed={false} />);
		const span = screen.getByTitle(ADDR_A);
		expect(span).toBeTruthy();
	});

	it("renders all validators when none are active", () => {
		render(<ValidatorList all={[ADDR_A, ADDR_B, ADDR_C]} active={[]} mapInfo={mapInfo} completed={false} />);
		expect(screen.getByText("Alice ⏳")).toBeTruthy();
		expect(screen.getByText("Bob ⏳")).toBeTruthy();
		expect(screen.getByText("Carol ⏳")).toBeTruthy();
	});

	it("renders nothing when both all and active are empty", () => {
		const { container } = render(<ValidatorList all={[]} active={[]} mapInfo={mapInfo} completed={false} />);
		expect(container.textContent).toBe("");
	});
});
