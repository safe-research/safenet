// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { StatusBadge } from "./StatusBadge";

afterEach(cleanup);

describe("StatusBadge", () => {
	it("renders a different badge for each status", () => {
		const { container: attested } = render(<StatusBadge status="ATTESTED" />);
		const { container: proposed } = render(<StatusBadge status="PROPOSED" />);
		const { container: timedOut } = render(<StatusBadge status="TIMED_OUT" />);
		expect(attested.innerHTML).not.toBe(proposed.innerHTML);
		expect(attested.innerHTML).not.toBe(timedOut.innerHTML);
		expect(proposed.innerHTML).not.toBe(timedOut.innerHTML);
	});

	it("renders ATTESTED text for ATTESTED status", () => {
		render(<StatusBadge status="ATTESTED" />);
		expect(screen.getByText("ATTESTED")).toBeTruthy();
	});

	it("renders PROPOSED text for PROPOSED status", () => {
		render(<StatusBadge status="PROPOSED" />);
		expect(screen.getByText("PROPOSED")).toBeTruthy();
	});

	it("renders TIMED OUT text for TIMED_OUT status", () => {
		render(<StatusBadge status="TIMED_OUT" />);
		expect(screen.getByText("TIMED OUT")).toBeTruthy();
	});
});
