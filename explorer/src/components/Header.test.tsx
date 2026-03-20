// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { zeroHash } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tanstack/react-router", () => ({
	Link: ({ children, to, onClick }: { children: React.ReactNode; to: string; onClick?: () => void }) => (
		<a href={to} onClick={onClick}>
			{children}
		</a>
	),
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
		const exploreLink = screen.getAllByRole("link", { name: "Explore" })[0];
		expect(exploreLink.getAttribute("href")).toBe("/");
	});

	it("renders Settings nav link pointing to /settings", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		const settingsLink = screen.getAllByRole("link", { name: "Settings" })[0];
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

	it("renders Block, Epoch and GroupId status items", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		expect(screen.getByText(/Block:/)).toBeTruthy();
		expect(screen.getByText(/Epoch:/)).toBeTruthy();
		expect(screen.getByText(/GroupId:/)).toBeTruthy();
		// Both epoch and groupId link to /epoch
		const epochLinks = screen.getAllByRole("link", { name: /^(7|0x00000000)/ });
		for (const link of epochLinks) {
			expect(link.getAttribute("href")).toBe("/epoch");
		}
	});

	it("hamburger button toggles mobile nav visibility", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);

		// Mobile menu is closed initially
		expect(screen.queryByRole("button", { name: "Open menu" })).toBeTruthy();
		expect(screen.queryByRole("button", { name: "Close menu" })).toBeNull();

		// Open the menu
		fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
		expect(screen.getByRole("button", { name: "Close menu" })).toBeTruthy();
		// Mobile nav links are now rendered (in addition to the always-present desktop ones)
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(2);

		// Close the menu
		fireEvent.click(screen.getByRole("button", { name: "Close menu" }));
		expect(screen.getByRole("button", { name: "Open menu" })).toBeTruthy();
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(1);
	});

	it("mobile nav closes when a link is clicked", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);

		fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(2);

		// Click the second (mobile) Explore link
		fireEvent.click(screen.getAllByRole("link", { name: "Explore" })[1]);
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(1);
	});
});
