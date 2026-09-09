// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { StatusBadge } from "./StatusBadge";

afterEach(cleanup);

describe("StatusBadge", () => {
	it.each([
		{ status: "TIMED_OUT" as const, text: "TIMED OUT", expectedClass: "bg-error" },
		{ status: "ATTESTED" as const, text: "ATTESTED", expectedClass: "bg-positive" },
		{ status: "DENIED" as const, text: "DENIED", expectedClass: "bg-error" },
		{ status: "APPROVED" as const, text: "APPROVED", expectedClass: "bg-info" },
		{ status: "PROPOSED" as const, text: "PROPOSED", expectedClass: "bg-pending" },
	])("renders $status with correct label and variant class", ({ status, text, expectedClass }) => {
		const { container } = render(<StatusBadge status={status} />);
		expect(screen.getByText(text)).toBeTruthy();
		expect((container.firstChild as HTMLElement).className).toContain(expectedClass);
	});
});
