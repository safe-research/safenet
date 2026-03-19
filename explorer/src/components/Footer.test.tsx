// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import Footer from "./Footer";

afterEach(cleanup);

describe("Footer", () => {
	it("renders copyright text", () => {
		render(<Footer />);
		expect(screen.getByText(/© Safenet \/ Safe Ecosystem Foundation/)).toBeTruthy();
	});

	it("renders Docs link with correct href and target blank", () => {
		render(<Footer />);
		const docsLink = screen.getByRole("link", { name: "Docs ↗" });
		expect(docsLink.getAttribute("href")).toBe("https://docs.safefoundation.org/safenet");
		expect(docsLink.getAttribute("target")).toBe("_blank");
		expect(docsLink.getAttribute("rel")).toBe("noopener noreferrer");
	});

	it("renders Terms, Privacy and Imprint as plain text when URLs are empty", () => {
		render(<Footer />);
		// When URL is empty, FooterLink renders a <span> — no <a> role
		expect(screen.queryByRole("link", { name: "Terms" })).toBeNull();
		expect(screen.queryByRole("link", { name: "Privacy" })).toBeNull();
		expect(screen.queryByRole("link", { name: "Imprint" })).toBeNull();
		// But the text should still be present
		expect(screen.getByText("Terms")).toBeTruthy();
		expect(screen.getByText("Privacy")).toBeTruthy();
		expect(screen.getByText("Imprint")).toBeTruthy();
	});

	it("renders footer navigation landmark", () => {
		render(<Footer />);
		expect(screen.getByRole("contentinfo")).toBeTruthy();
	});
});
