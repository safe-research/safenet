// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Address } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SearchBar } from "./SearchBar";

const mockNavigate = vi.fn();

vi.mock("@tanstack/react-router", () => ({
	useNavigate: () => mockNavigate,
}));

vi.mock("@/lib/chains", () => ({
	SAFE_SERVICE_CHAINS: {
		ethereum: { id: "1", name: "Ethereum" },
	},
}));

afterEach(() => {
	cleanup();
	mockNavigate.mockClear();
});

const SAFE_ADDRESS = "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF" as Address;
const CHECKSUMMED = "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF";
const TX_HASH = "0xabc123def456abc123def456abc123def456abc123def456abc123def456abc1";

function renderSearchBar(selectedNetwork = "1") {
	return render(<SearchBar selectedNetwork={selectedNetwork} onSelectNetwork={vi.fn()} />);
}

function clickSearchIcon() {
	// The search trigger is an SVG with onClick, not a button element
	const icon = document.querySelector("svg");
	if (!icon) throw new Error("Search icon not found");
	fireEvent.click(icon);
}

describe("SearchBar", () => {
	it("navigates to /safe with checksummed address when input is a valid address", () => {
		renderSearchBar();
		fireEvent.change(screen.getByRole("textbox"), { target: { value: SAFE_ADDRESS } });
		clickSearchIcon();
		expect(mockNavigate).toHaveBeenCalledWith({
			to: "/safe",
			search: { safeAddress: CHECKSUMMED, chainId: "1" },
		});
	});

	it("navigates to /safeTx when input is not an address", () => {
		renderSearchBar();
		fireEvent.change(screen.getByRole("textbox"), { target: { value: TX_HASH } });
		clickSearchIcon();
		expect(mockNavigate).toHaveBeenCalledWith({
			to: "/safeTx",
			search: { chainId: "1", safeTxHash: TX_HASH },
		});
	});

	it("does not navigate when input is empty", () => {
		renderSearchBar();
		clickSearchIcon();
		expect(mockNavigate).not.toHaveBeenCalled();
	});

	it("does not navigate when input is only whitespace", () => {
		renderSearchBar();
		fireEvent.change(screen.getByRole("textbox"), { target: { value: "   " } });
		clickSearchIcon();
		expect(mockNavigate).not.toHaveBeenCalled();
	});

	it("navigates to /safe on Enter key for address input", () => {
		renderSearchBar();
		const input = screen.getByRole("textbox");
		fireEvent.change(input, { target: { value: SAFE_ADDRESS } });
		fireEvent.keyDown(input, { key: "Enter" });
		expect(mockNavigate).toHaveBeenCalledWith({
			to: "/safe",
			search: { safeAddress: CHECKSUMMED, chainId: "1" },
		});
	});

	it("navigates to /safeTx on Enter key for non-address input", () => {
		renderSearchBar();
		const input = screen.getByRole("textbox");
		fireEvent.change(input, { target: { value: TX_HASH } });
		fireEvent.keyDown(input, { key: "Enter" });
		expect(mockNavigate).toHaveBeenCalledWith({
			to: "/safeTx",
			search: { chainId: "1", safeTxHash: TX_HASH },
		});
	});

	it("passes the selectedNetwork as chainId when navigating", () => {
		renderSearchBar("11155111");
		fireEvent.change(screen.getByRole("textbox"), { target: { value: TX_HASH } });
		clickSearchIcon();
		expect(mockNavigate).toHaveBeenCalledWith({
			to: "/safeTx",
			search: { chainId: "11155111", safeTxHash: TX_HASH },
		});
	});
});
