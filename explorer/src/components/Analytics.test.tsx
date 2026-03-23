// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import type React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let mockInit: ReturnType<typeof vi.fn>;
let Analytics: React.ComponentType;

beforeEach(async () => {
	// Reset the module registry so the module-level `initialized` flag in
	// Analytics.tsx is reset to false for each test.
	vi.resetModules();
	mockInit = vi.fn();
	vi.doMock("@plausible-analytics/tracker", () => ({ init: mockInit }));
	const module = await import("./Analytics.tsx");
	Analytics = module.default;
});

afterEach(() => {
	cleanup();
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
