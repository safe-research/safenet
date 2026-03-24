// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let mockInit: ReturnType<typeof vi.fn>;

beforeEach(() => {
	// Reset the module registry so the module-level init() call in
	// Analytics.tsx is re-evaluated fresh for each test.
	vi.resetModules();
	mockInit = vi.fn();
	vi.doMock("@plausible-analytics/tracker", () => ({ init: mockInit }));
});

afterEach(() => {
	cleanup();
	vi.unstubAllEnvs();
});

describe("Analytics", () => {
	it("does not call init when VITE_PLAUSIBLE_DOMAIN is not set", async () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "");
		await import("./Analytics.tsx");
		expect(mockInit).not.toHaveBeenCalled();
	});

	it("calls init with the configured domain", async () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		await import("./Analytics.tsx");
		expect(mockInit).toHaveBeenCalledOnce();
		expect(mockInit).toHaveBeenCalledWith({ domain: "explorer.safenet.io" });
	});

	it("passes a custom endpoint when VITE_PLAUSIBLE_ENDPOINT is set", async () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		vi.stubEnv("VITE_PLAUSIBLE_ENDPOINT", "https://plausible.example.com/api/event");
		await import("./Analytics.tsx");
		expect(mockInit).toHaveBeenCalledWith({
			domain: "explorer.safenet.io",
			endpoint: "https://plausible.example.com/api/event",
		});
	});

	it("calls init only once on re-render", async () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		const { default: Analytics } = await import("./Analytics.tsx");
		render(<Analytics />);
		render(<Analytics />);
		expect(mockInit).toHaveBeenCalledOnce();
	});

	it("renders nothing", async () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		const { default: Analytics } = await import("./Analytics.tsx");
		const { container } = render(<Analytics />);
		expect(container.firstChild).toBeNull();
	});
});
