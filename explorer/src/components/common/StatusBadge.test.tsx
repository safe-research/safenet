// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { StatusBadge } from "./StatusBadge";

afterEach(cleanup);

describe("StatusBadge", () => {
	it("renders PROPOSED when attested is false", () => {
		render(<StatusBadge attested={false} />);
		expect(screen.getByText("PROPOSED")).toBeTruthy();
	});

	it("renders ATTESTED when attested is true", () => {
		render(<StatusBadge attested={true} />);
		expect(screen.getByText("ATTESTED")).toBeTruthy();
	});
});
