// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { NetworkBadge } from "./NetworkBadge";

afterEach(cleanup);

describe("NetworkBadge", () => {
	it("renders ETH for Ethereum mainnet (chainId 1)", () => {
		render(<NetworkBadge chainId={1n} />);
		expect(screen.getByText("ETH")).toBeTruthy();
	});

	it("renders GNO for Gnosis Chain (chainId 100)", () => {
		render(<NetworkBadge chainId={100n} />);
		expect(screen.getByText("GNO")).toBeTruthy();
	});

	it("renders BASE for Base (chainId 8453)", () => {
		render(<NetworkBadge chainId={8453n} />);
		expect(screen.getByText("BASE")).toBeTruthy();
	});

	it("renders the raw chain ID when the chain is unknown", () => {
		render(<NetworkBadge chainId={999n} />);
		expect(screen.getByText("999")).toBeTruthy();
	});
});
