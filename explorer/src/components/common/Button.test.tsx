// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Button } from "./Button";

afterEach(cleanup);

describe("Button", () => {
	it("renders with default primary variant", () => {
		render(<Button>Click me</Button>);
		expect(screen.getByRole("button", { name: "Click me" })).toBeTruthy();
	});

	it("renders with ghost variant", () => {
		const { container } = render(<Button variant="ghost">Ghost</Button>);
		expect((container.firstChild as HTMLElement).className).toContain("hover:underline");
	});

	it("renders with icon variant", () => {
		const { container } = render(<Button variant="icon">Icon</Button>);
		expect((container.firstChild as HTMLElement).className).toContain("inline-flex");
	});

	it("renders with primary variant classes", () => {
		const { container } = render(<Button variant="primary">Primary</Button>);
		expect((container.firstChild as HTMLElement).className).toContain("bg-button");
	});

	it("merges className via cn()", () => {
		const { container } = render(<Button className="extra-class">X</Button>);
		expect((container.firstChild as HTMLElement).className).toContain("extra-class");
	});

	it("is disabled when disabled prop is set", () => {
		render(<Button disabled>Disabled</Button>);
		expect((screen.getByRole("button") as HTMLButtonElement).disabled).toBe(true);
	});

	it("forwards onClick handler", async () => {
		let clicked = false;
		render(
			<Button
				onClick={() => {
					clicked = true;
				}}
			>
				Click
			</Button>,
		);
		screen.getByRole("button").click();
		expect(clicked).toBe(true);
	});
});
