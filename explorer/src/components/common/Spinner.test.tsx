// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Spinner } from "./Spinner";

afterEach(cleanup);

describe("Spinner", () => {
	it("renders an svg with role img", () => {
		render(<Spinner />);
		expect(screen.getByRole("img", { name: "Loading" })).toBeTruthy();
	});

	it("applies animate-spin class by default", () => {
		render(<Spinner />);
		expect(screen.getByRole("img").getAttribute("class")).toContain("animate-spin");
	});

	it("merges className via cn()", () => {
		render(<Spinner className="extra-class" />);
		expect(screen.getByRole("img").getAttribute("class")).toContain("extra-class");
	});
});
