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

	it("renders with positive variant", () => {
		render(<Badge variant="positive">OK</Badge>);
		expect(screen.getByText("OK")).toBeTruthy();
	});

	it("renders with pending variant", () => {
		render(<Badge variant="pending">PENDING</Badge>);
		expect(screen.getByText("PENDING")).toBeTruthy();
	});

	it("renders with error variant", () => {
		render(<Badge variant="error">ERROR</Badge>);
		expect(screen.getByText("ERROR")).toBeTruthy();
	});

	it("renders with warning variant", () => {
		render(<Badge variant="warning">WARN</Badge>);
		expect(screen.getByText("WARN")).toBeTruthy();
	});

	it("renders with neutral variant", () => {
		render(<Badge variant="neutral">NEUTRAL</Badge>);
		expect(screen.getByText("NEUTRAL")).toBeTruthy();
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
