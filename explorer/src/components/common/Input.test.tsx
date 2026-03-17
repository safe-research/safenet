// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Input } from "./Input";

afterEach(cleanup);

describe("Input", () => {
	it("renders an input element", () => {
		render(<Input />);
		expect(screen.getByRole("textbox")).toBeTruthy();
	});

	it("merges className via cn()", () => {
		const { container } = render(<Input className="extra-class" />);
		expect((container.firstChild as HTMLElement).className).toContain("extra-class");
	});

	it("applies base border and background classes", () => {
		const { container } = render(<Input />);
		const el = container.firstChild as HTMLElement;
		expect(el.className).toContain("border-surface-outline");
		expect(el.className).toContain("bg-surface-1");
	});

	it("is disabled when disabled prop is set", () => {
		render(<Input disabled />);
		expect((screen.getByRole("textbox") as HTMLInputElement).disabled).toBe(true);
	});

	it("forwards placeholder prop", () => {
		render(<Input placeholder="Enter value" />);
		expect((screen.getByPlaceholderText("Enter value") as HTMLInputElement).placeholder).toBe("Enter value");
	});

	it("forwards type prop", () => {
		render(<Input type="email" />);
		expect((screen.getByRole("textbox") as HTMLInputElement).type).toBe("email");
	});
});
