// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { InfoPopover } from "./InfoPopover";

afterEach(cleanup);

describe("InfoPopover", () => {
	it("shows content when trigger is clicked and hides it when clicked again", () => {
		render(<InfoPopover trigger={<span>click me</span>}>popover content</InfoPopover>);
		expect(screen.getByText("click me")).toBeTruthy();
		expect(screen.queryByText("popover content")).toBeNull();
		fireEvent.click(screen.getByRole("button"));
		expect(screen.getByText("popover content")).toBeTruthy();
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
