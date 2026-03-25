// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { InfoPopover } from "./InfoPopover";

afterEach(cleanup);

describe("InfoPopover", () => {
	it("renders the trigger", () => {
		render(<InfoPopover trigger={<span>click me</span>}>content</InfoPopover>);
		expect(screen.getByText("click me")).toBeTruthy();
	});

	it("hides content by default", () => {
		render(<InfoPopover trigger={<span>trigger</span>}>hidden content</InfoPopover>);
		expect(screen.queryByText("hidden content")).toBeNull();
	});

	it("shows content after clicking the trigger", () => {
		render(<InfoPopover trigger={<span>trigger</span>}>popover content</InfoPopover>);
		fireEvent.click(screen.getByRole("button"));
		expect(screen.getByText("popover content")).toBeTruthy();
	});

	it("hides content after clicking the trigger again", () => {
		render(<InfoPopover trigger={<span>trigger</span>}>popover content</InfoPopover>);
		fireEvent.click(screen.getByRole("button"));
		fireEvent.click(screen.getByRole("button"));
		expect(screen.queryByText("popover content")).toBeNull();
	});

	it("closes when clicking outside", () => {
		render(
			<div>
				<InfoPopover trigger={<span>trigger</span>}>popover content</InfoPopover>
				<div data-testid="outside">outside</div>
			</div>,
		);
		fireEvent.click(screen.getByRole("button"));
		expect(screen.getByText("popover content")).toBeTruthy();
		fireEvent.mouseDown(screen.getByTestId("outside"));
		expect(screen.queryByText("popover content")).toBeNull();
	});
});
