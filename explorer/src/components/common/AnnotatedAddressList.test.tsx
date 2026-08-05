// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Address } from "viem";
import { afterEach, describe, expect, it } from "vitest";
import { AnnotatedAddressList } from "./AnnotatedAddressList";

afterEach(cleanup);

const ADDR_A = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" as Address;
const ADDR_B = "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB" as Address;
const ADDR_C = "0xCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC" as Address;

const LABEL: Record<Address, string> = { [ADDR_A]: "Alice", [ADDR_B]: "Bob", [ADDR_C]: "Carol" };
const label = (address: Address) => LABEL[address];

describe("AnnotatedAddressList", () => {
	it("renders the label returns for each account", () => {
		render(<AnnotatedAddressList accounts={[ADDR_A, ADDR_B]} label={label} />);
		expect(screen.getByText("Alice")).toBeTruthy();
		expect(screen.getByText("Bob")).toBeTruthy();
	});

	it("sorts active accounts first, alphabetically within each group", () => {
		const { container } = render(
			<AnnotatedAddressList accounts={[ADDR_C, ADDR_A, ADDR_B]} active={[ADDR_B]} label={label} />,
		);
		const text = container.textContent ?? "";
		expect(text.indexOf("Bob")).toBeLessThan(text.indexOf("Alice"));
		expect(text.indexOf("Alice")).toBeLessThan(text.indexOf("Carol"));
	});

	it("treats all accounts as active (purely alphabetical order) when active is omitted", () => {
		const { container } = render(<AnnotatedAddressList accounts={[ADDR_C, ADDR_A]} label={label} />);
		const text = container.textContent ?? "";
		expect(text.indexOf("Alice")).toBeLessThan(text.indexOf("Carol"));
	});

	it("shows the address and a copy button in a popover when clicking an account label", () => {
		render(<AnnotatedAddressList accounts={[ADDR_A]} label={label} />);
		fireEvent.click(screen.getByText("Alice"));
		expect(screen.getByText(ADDR_A)).toBeTruthy();
		expect(screen.getByRole("button", { name: "Copy to clipboard" })).toBeTruthy();
	});

	it("renders popoverContent for the clicked account", () => {
		render(
			<AnnotatedAddressList
				accounts={[ADDR_A]}
				label={label}
				popoverContent={(address) => <span>note for {address}</span>}
			/>,
		);
		fireEvent.click(screen.getByText("Alice"));
		expect(screen.getByText(`note for ${ADDR_A}`)).toBeTruthy();
	});

	it("renders nothing when accounts is empty", () => {
		const { container } = render(<AnnotatedAddressList accounts={[]} label={label} />);
		expect(container.textContent).toBe("");
	});

	it("still renders an active account that isn't in accounts", () => {
		render(<AnnotatedAddressList accounts={[ADDR_A]} active={[ADDR_A, ADDR_B]} label={label} />);
		expect(screen.getByText("Alice")).toBeTruthy();
		expect(screen.getByText("Bob")).toBeTruthy();
	});
});
