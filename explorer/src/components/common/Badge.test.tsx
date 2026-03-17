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
		{ variant: "positive" as const, text: "OK" },
		{ variant: "pending" as const, text: "PENDING" },
		{ variant: "error" as const, text: "ERROR" },
		{ variant: "warning" as const, text: "WARN" },
		{ variant: "neutral" as const, text: "NEUTRAL" },
	])("renders with $variant variant", ({ variant, text }) => {
		render(<Badge variant={variant}>{text}</Badge>);
		expect(screen.getByText(text)).toBeTruthy();
	});

	it("merges className via cn()", () => {
		const { container } = render(<Badge className="extra-class">X</Badge>);
		expect((container.firstChild as HTMLElement).className).toContain("extra-class");
	});

	it("applies variant classes when variant is set", () => {
		const { container } = render(<Badge variant="positive">X</Badge>);
		expect((container.firstChild as HTMLElement).className).toContain("bg-positive");
	});

	it("applies inline style when bgColor is provided", () => {
		const { container } = render(<Badge bgColor="#ff0000">X</Badge>);
		expect((container.firstChild as HTMLElement).getAttribute("style")).toContain("background-color");
	});
});
