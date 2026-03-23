// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mockInit = vi.fn();
vi.mock("@plausible-analytics/tracker", () => ({ init: mockInit }));

// Re-import after mock is set up
const { default: Analytics } = await import("./Analytics.tsx");

afterEach(() => {
	cleanup();
	mockInit.mockReset();
	vi.unstubAllEnvs();
});

describe("Analytics", () => {
	it("does not call init when VITE_PLAUSIBLE_DOMAIN is not set", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "");
		render(<Analytics />);
		expect(mockInit).not.toHaveBeenCalled();
	});

	it("calls init with the configured domain", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		render(<Analytics />);
		expect(mockInit).toHaveBeenCalledOnce();
		expect(mockInit).toHaveBeenCalledWith({ domain: "explorer.safenet.io" });
	});

	it("passes a custom endpoint when VITE_PLAUSIBLE_ENDPOINT is set", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		vi.stubEnv("VITE_PLAUSIBLE_ENDPOINT", "https://plausible.example.com/api/event");
		render(<Analytics />);
		expect(mockInit).toHaveBeenCalledWith({
			domain: "explorer.safenet.io",
			endpoint: "https://plausible.example.com/api/event",
		});
	});

	it("calls init only once on re-render", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		const { rerender } = render(<Analytics />);
		rerender(<Analytics />);
		expect(mockInit).toHaveBeenCalledOnce();
	});

	it("renders nothing", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		const { container } = render(<Analytics />);
		expect(container.firstChild).toBeNull();
	});
});
