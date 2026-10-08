// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LegalPage } from "@/lib/legal";
import Footer from "./Footer";

vi.mock("@tanstack/react-router", () => ({
	Link: ({ children, to, className }: { children: React.ReactNode; to: string; className?: string }) => (
		<a href={to} className={className}>
			{children}
		</a>
	),
}));

const legalPages = vi.hoisted((): LegalPage[] => []);

vi.mock("@/lib/legal", () => ({ LEGAL_PAGES: legalPages }));

function setLegalPages({ termsBundled = false } = {}) {
	legalPages.splice(
		0,
		legalPages.length,
		{ label: "Terms", to: "/terms", bundled: termsBundled, url: __TERMS_URL__ },
		{ label: "Privacy", to: "/privacy", bundled: false, url: __PRIVACY_URL__ },
		{ label: "Imprint", to: "/imprint", bundled: false, url: __IMPRINT_URL__ },
	);
}

afterEach(cleanup);

describe("Footer", () => {
	it("renders Docs link with correct href and target blank", () => {
		render(<Footer />);
		const docsLink = screen.getByRole("link", { name: "Docs ↗" });
		expect(docsLink.getAttribute("href")).toBe("https://docs.safefoundation.org/safenet");
		expect(docsLink.getAttribute("target")).toBe("_blank");
		expect(docsLink.getAttribute("rel")).toBe("noopener noreferrer");
	});

	it("renders Terms, Privacy and Imprint as links with configured URLs", () => {
		setLegalPages();
		render(<Footer />);
		const termsLink = screen.getByRole("link", { name: "Terms" });
		expect(termsLink.getAttribute("href")).toBe("https://test.example/terms");
		expect(termsLink.getAttribute("target")).toBe("_blank");

		const privacyLink = screen.getByRole("link", { name: "Privacy" });
		expect(privacyLink.getAttribute("href")).toBe("https://test.example/privacy");
		expect(privacyLink.getAttribute("target")).toBe("_blank");

		const imprintLink = screen.getByRole("link", { name: "Imprint" });
		expect(imprintLink.getAttribute("href")).toBe("https://test.example/imprint");
		expect(imprintLink.getAttribute("target")).toBe("_blank");
	});

	it("links to the in-app page when its content is bundled", () => {
		setLegalPages({ termsBundled: true });
		render(<Footer />);
		const termsLink = screen.getByRole("link", { name: "Terms" });
		expect(termsLink.getAttribute("href")).toBe("/terms");
		expect(termsLink.getAttribute("target")).toBeNull();

		const privacyLink = screen.getByRole("link", { name: "Privacy" });
		expect(privacyLink.getAttribute("href")).toBe("https://test.example/privacy");
		expect(privacyLink.getAttribute("target")).toBe("_blank");
	});
});
