// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Analytics from "./Analytics.tsx";

afterEach(() => {
	cleanup();
	vi.unstubAllEnvs();
});

describe("Analytics", () => {
	it("renders nothing when VITE_PLAUSIBLE_DOMAIN is not set", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "");
		const { container } = render(<Analytics />);
		expect(container.firstChild).toBeNull();
	});

	it("renders a script tag when VITE_PLAUSIBLE_DOMAIN is set", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		const { container } = render(<Analytics />);
		const script = container.querySelector("script");
		expect(script).not.toBeNull();
		expect(script?.getAttribute("data-domain")).toBe("explorer.safenet.io");
		expect(script?.getAttribute("src")).toBe("https://plausible.io/js/script.js");
		expect(script?.hasAttribute("defer")).toBe(true);
	});

	it("uses custom script URL when VITE_PLAUSIBLE_SCRIPT_URL is set", () => {
		vi.stubEnv("VITE_PLAUSIBLE_DOMAIN", "explorer.safenet.io");
		vi.stubEnv("VITE_PLAUSIBLE_SCRIPT_URL", "https://plausible.example.com/js/script.js");
		const { container } = render(<Analytics />);
		const script = container.querySelector("script");
		expect(script?.getAttribute("src")).toBe("https://plausible.example.com/js/script.js");
	});
});
