// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CopyButton } from "./CopyButton";

afterEach(cleanup);

describe("CopyButton", () => {
	beforeEach(() => {
		Object.assign(navigator, {
			clipboard: {
				writeText: vi.fn().mockResolvedValue(undefined),
			},
		});
	});

	it("calls navigator.clipboard.writeText with the correct value", async () => {
		render(<CopyButton value="0xdeadbeef" />);
		fireEvent.click(screen.getByRole("button"));
		expect(navigator.clipboard.writeText).toHaveBeenCalledWith("0xdeadbeef");
	});

	it("shows checkmark confirmation after copy", async () => {
		render(<CopyButton value="hello" />);
		fireEvent.click(screen.getByRole("button"));
		await waitFor(() => expect(screen.getByRole("button").getAttribute("aria-label")).toBe("Copied"));
	});

	it("shows copy icon initially", () => {
		render(<CopyButton value="test" />);
		expect(screen.getByRole("button").getAttribute("aria-label")).toBe("Copy to clipboard");
	});
});
