// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import Footer from "./Footer";

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
});
