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
	it("renders Explore nav link", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		expect(screen.getByRole("link", { name: "Explore" })).toBeTruthy();
	});

	it("renders Settings nav link", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		expect(screen.getByRole("link", { name: "Settings" })).toBeTruthy();
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
		expect(screen.getByText(/Epoch: 7/)).toBeTruthy();
	});

	it("does not render beta warning banner", async () => {
		const { default: Header } = await import("./Header");
		render(<Header />);
		expect(screen.queryByRole("alert")).toBeNull();
		expect(screen.queryByText(/experimental beta/i)).toBeNull();
	});
});
