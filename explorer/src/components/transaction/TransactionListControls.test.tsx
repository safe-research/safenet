// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { formatLastUpdated, TransactionListControls } from "./TransactionListControls";

afterEach(cleanup);

describe("formatLastUpdated", () => {
	it("returns '—' when dataUpdatedAt is 0", () => {
		expect(formatLastUpdated(0)).toBe("—");
	});

	it("returns a formatted time string when dataUpdatedAt is set", () => {
		const result = formatLastUpdated(Date.now());
		expect(result).not.toBe("—");
		expect(result.length).toBeGreaterThan(0);
	});
});

describe("TransactionListControls", () => {
	const defaultProps = {
		isFetching: false,
		dataUpdatedAt: 0,
		autoRefresh: true,
		onRefetch: vi.fn(),
		onToggleAutoRefresh: vi.fn(),
	};

	it("refresh button calls onRefetch when clicked", () => {
		const onRefetch = vi.fn();
		render(<TransactionListControls {...defaultProps} onRefetch={onRefetch} />);
		fireEvent.click(screen.getByRole("button", { name: /refresh now/i }));
		expect(onRefetch).toHaveBeenCalledOnce();
	});

	it("toggle button calls onToggleAutoRefresh when clicked", () => {
		const onToggleAutoRefresh = vi.fn();
		render(<TransactionListControls {...defaultProps} onToggleAutoRefresh={onToggleAutoRefresh} />);
		fireEvent.click(screen.getByRole("button", { pressed: true }));
		expect(onToggleAutoRefresh).toHaveBeenCalledOnce();
	});

	it("shows ON when autoRefresh is true", () => {
		render(<TransactionListControls {...defaultProps} autoRefresh={true} />);
		expect(screen.getByRole("button", { pressed: true }).textContent).toBe("ON");
	});

	it("shows OFF when autoRefresh is false", () => {
		render(<TransactionListControls {...defaultProps} autoRefresh={false} />);
		expect(screen.getByRole("button", { pressed: false }).textContent).toBe("OFF");
	});

	it("refresh button is disabled while isFetching", () => {
		render(<TransactionListControls {...defaultProps} isFetching={true} />);
		const button = screen.getByRole("button", { name: /refresh now/i }) as HTMLButtonElement;
		expect(button.disabled).toBe(true);
	});

	it("shows spinner while isFetching", () => {
		render(<TransactionListControls {...defaultProps} isFetching={true} />);
		expect(document.querySelector("svg.animate-spin")).toBeTruthy();
	});

	it("does not show spinner when not fetching", () => {
		render(<TransactionListControls {...defaultProps} isFetching={false} />);
		expect(document.querySelector("svg.animate-spin")).toBeNull();
	});

	it("last updated shows '—' when dataUpdatedAt is 0", () => {
		render(<TransactionListControls {...defaultProps} dataUpdatedAt={0} />);
		expect(screen.getByText(/Last updated:/).textContent).toContain("—");
	});

	it("last updated shows formatted time when dataUpdatedAt is set", () => {
		render(<TransactionListControls {...defaultProps} dataUpdatedAt={Date.now()} />);
		const text = screen.getByText(/Last updated:/).textContent ?? "";
		expect(text).not.toContain("—");
		expect(text.length).toBeGreaterThan("Last updated: ".length);
	});
});
