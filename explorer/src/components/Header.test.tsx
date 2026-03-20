// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { zeroHash } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tanstack/react-router", () => ({
	Link: ({ children, to }: { children: React.ReactNode; to: string }) => <a href={to}>{children}</a>,
}));

const mockUseConsensusState = vi.hoisted(() =>
	vi.fn(() => ({
		data: { currentBlock: 42n, currentEpoch: 7n, currentGroupId: zeroHash },
	})),
);

vi.mock("@/hooks/useConsensusState", () => ({
	useConsensusState: mockUseConsensusState,
}));

afterEach(() => {
	cleanup();
	mockUseConsensusState.mockClear();
});

describe("Header", () => {
	it("renders Explore nav link pointing to /", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		const exploreLink = screen.getByRole("link", { name: "Explore" });
		expect(exploreLink.getAttribute("href")).toBe("/");
	});

	it("renders Settings nav link pointing to /settings", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		const settingsLink = screen.getByRole("link", { name: "Settings" });
		expect(settingsLink.getAttribute("href")).toBe("/settings");
	});

	it("renders Docs external link opening in new tab", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		const docsLink = screen.getByRole("link", { name: "Docs ↗" });
		expect(docsLink.getAttribute("href")).toBe("https://docs.safefoundation.org/safenet");
		expect(docsLink.getAttribute("target")).toBe("_blank");
		expect(docsLink.getAttribute("rel")).toBe("noopener noreferrer");
	});

	it("renders status info from consensus state", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		expect(screen.getByText(/Block: 42/)).toBeTruthy();
		expect(screen.getByText(/Epoch:/)).toBeTruthy();
		// Both epoch and groupId link to /epoch
		const epochLinks = screen.getAllByRole("link", { name: /^(7|0x00000000)/ });
		for (const link of epochLinks) {
			expect(link.getAttribute("href")).toBe("/epoch");
		}
	});
});
