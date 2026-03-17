// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Badge } from "./Badge";

afterEach(cleanup);

describe("Badge", () => {
	it("renders children", () => {
		render(<Badge>LABEL</Badge>);
		expect(screen.getByText("LABEL")).toBeTruthy();
	});

	it.each([
		{ variant: "positive" as const, text: "OK", expectedClass: "bg-positive" },
		{ variant: "pending" as const, text: "PENDING", expectedClass: "bg-pending" },
		{ variant: "error" as const, text: "ERROR", expectedClass: "bg-error-surface" },
		{ variant: "warning" as const, text: "WARN", expectedClass: "bg-warning-surface" },
		{ variant: "neutral" as const, text: "NEUTRAL", expectedClass: "bg-surface-0" },
	])("renders with $variant variant and applies correct class", ({ variant, text, expectedClass }) => {
		const { container } = render(<Badge variant={variant}>{text}</Badge>);
		expect(screen.getByText(text)).toBeTruthy();
		expect((container.firstChild as HTMLElement).className).toContain(expectedClass);
	});

	it("provided className is applied", () => {
		const { container } = render(<Badge className="extra-class">X</Badge>);
		expect((container.firstChild as HTMLElement).className).toContain("extra-class");
	});

	it("applies the provided bgColor as inline style", () => {
		const { container } = render(<Badge bgColor="#ff0000">X</Badge>);
		expect((container.firstChild as HTMLElement).style.backgroundColor).toBe("rgb(255, 0, 0)");
	});
});
