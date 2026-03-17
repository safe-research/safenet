// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Label } from "./Label";

afterEach(cleanup);

describe("Label", () => {
	it("renders children", () => {
		render(<Label>My label</Label>);
		expect(screen.getByText("My label")).toBeTruthy();
	});

	it("merges className via cn()", () => {
		const { container } = render(<Label className="extra-class">X</Label>);
		expect((container.firstChild as HTMLElement).className).toContain("extra-class");
	});

	it("applies base typography classes", () => {
		const { container } = render(<Label>X</Label>);
		const el = container.firstChild as HTMLElement;
		expect(el.className).toContain("text-sm");
		expect(el.className).toContain("font-medium");
	});

	it("forwards htmlFor prop", () => {
		render(<Label htmlFor="my-input">Label</Label>);
		expect(screen.getByText("Label").getAttribute("for")).toBe("my-input");
	});
});
