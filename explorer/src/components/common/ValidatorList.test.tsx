// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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

	it("shows the validator address and a copy button in a popover when clicking a validator label", () => {
		render(<ValidatorList all={[ADDR_A]} active={[ADDR_A]} mapInfo={mapInfo} completed={false} />);
		fireEvent.click(screen.getByText("Alice ✅"));
		expect(screen.getByText(ADDR_A)).toBeTruthy();
		expect(screen.getByRole("button", { name: "Copy to clipboard" })).toBeTruthy();
	});

	it("shows the full address in the popover when no validator info is available", () => {
		const noLabelMapInfo = createMapInfo(null);
		render(<ValidatorList all={[ADDR_A]} active={[ADDR_A]} mapInfo={noLabelMapInfo} completed={false} />);
		fireEvent.click(screen.getByRole("button"));
		expect(screen.getByText(ADDR_A)).toBeTruthy();
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
