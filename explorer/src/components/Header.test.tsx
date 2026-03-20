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
		// Desktop nav link is always in DOM
		const exploreLink = screen.getAllByRole("link", { name: "Explore" })[0];
		expect(exploreLink.getAttribute("href")).toBe("/");
	});

	it("renders Settings nav link pointing to /settings", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		const settingsLink = screen.getAllByRole("link", { name: "Settings" })[0];
		expect(settingsLink.getAttribute("href")).toBe("/settings");
	});

	it("renders Docs external link with correct href and target blank", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		// Desktop Docs link is always in DOM
		const docsLink = screen.getAllByRole("link", { name: "Docs ↗" })[0];
		expect(docsLink.getAttribute("href")).toBe("https://docs.safefoundation.org/safenet");
		expect(docsLink.getAttribute("target")).toBe("_blank");
		expect(docsLink.getAttribute("rel")).toBe("noopener noreferrer");
	});

	it("renders Block, Epoch and GroupId status items", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		// Desktop status row is always in DOM
		expect(screen.getAllByText(/Block: 42/)[0]).toBeTruthy();
		const epochLinks = screen.getAllByRole("link", { name: "7" });
		expect(epochLinks[0].getAttribute("href")).toBe("/epoch");
	});

	it("hamburger button toggles mobile nav", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);

		expect(screen.getByRole("button", { name: "Open menu" })).toBeTruthy();

		// Open — mobile dropdown adds a second set of nav links + Docs
		fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
		expect(screen.getByRole("button", { name: "Close menu" })).toBeTruthy();
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(2);
		expect(screen.getAllByRole("link", { name: "Docs ↗" })).toHaveLength(2);
		// Stats appear in mobile dropdown (in addition to hidden desktop row)
		expect(screen.getAllByText(/Block: 42/)).toHaveLength(2);

		// Close
		fireEvent.click(screen.getByRole("button", { name: "Close menu" }));
		expect(screen.getByRole("button", { name: "Open menu" })).toBeTruthy();
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(1);
	});

	it("mobile nav closes when a link is clicked", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);

		fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(2);

		fireEvent.click(screen.getAllByRole("link", { name: "Explore" })[1]);
		expect(screen.getAllByRole("link", { name: "Explore" })).toHaveLength(1);
	});
});
