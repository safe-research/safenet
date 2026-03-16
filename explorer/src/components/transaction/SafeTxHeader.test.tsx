// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import type { Address, Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SafeTransaction } from "@/lib/consensus";
import { SafeTxHeader } from "./SafeTxHeader";

vi.mock("@/components/common/CopyButton", () => ({
	CopyButton: ({ value }: { value: string }) => <button type="button">{value}</button>,
}));

afterEach(cleanup);

const SAFE_TX_HASH = "0x9f1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab7a" as Hex;

const makeTransaction = (chainId: bigint): SafeTransaction => ({
	chainId,
	safe: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as Address,
	to: "0x0000000000000000000000000000000000000002" as Address,
	value: 0n,
	data: "0x" as Hex,
	operation: 0,
	safeTxGas: 0n,
	baseGas: 0n,
	gasPrice: 0n,
	gasToken: "0x0000000000000000000000000000000000000000" as Address,
	refundReceiver: "0x0000000000000000000000000000000000000000" as Address,
	nonce: 0n,
});

describe("SafeTxHeader", () => {
	it("shows Safe Wallet tx link when fromSafeApi=true and chain is supported", () => {
		const transaction = makeTransaction(1n); // mainnet
		render(<SafeTxHeader safeTxHash={SAFE_TX_HASH} transaction={transaction} fromSafeApi={true} />);
		const links = screen.getAllByRole("link", { name: /open in safe wallet/i });
		expect(links.length).toBeGreaterThanOrEqual(2);
		const txLink = links.find((l) => l.getAttribute("href")?.includes("/transactions/tx"));
		expect(txLink).toBeTruthy();
	});

	it("hides Safe Wallet tx link when fromSafeApi=false", () => {
		const transaction = makeTransaction(1n);
		render(<SafeTxHeader safeTxHash={SAFE_TX_HASH} transaction={transaction} fromSafeApi={false} />);
		const txLink = screen
			.queryAllByRole("link", { name: /open in safe wallet/i })
			.find((l) => l.getAttribute("href")?.includes("/transactions/tx"));
		expect(txLink).toBeUndefined();
	});

	it("still shows Safe address link when fromSafeApi=false and chain is supported", () => {
		const transaction = makeTransaction(1n);
		render(<SafeTxHeader safeTxHash={SAFE_TX_HASH} transaction={transaction} fromSafeApi={false} />);
		const links = screen.getAllByRole("link", { name: /open in safe wallet/i });
		expect(links.length).toBe(1);
		expect(links[0].getAttribute("href")).toContain("/balances");
	});

	it("hides all Safe Wallet links for unsupported chainId", () => {
		const transaction = makeTransaction(99999n); // unsupported chain
		render(<SafeTxHeader safeTxHash={SAFE_TX_HASH} transaction={transaction} fromSafeApi={true} />);
		const links = screen.queryAllByRole("link", { name: /open in safe wallet/i });
		expect(links.length).toBe(0);
	});

	it("shows the network badge with tooltip for supported chains", () => {
		const transaction = makeTransaction(8453n); // base
		render(<SafeTxHeader safeTxHash={SAFE_TX_HASH} transaction={transaction} fromSafeApi={false} />);
		expect(screen.getByText("BASE")).toBeTruthy();
		expect(screen.getByTitle("Base (chain id 8453)")).toBeTruthy();
	});

	it("shows the raw chainId for unsupported chains", () => {
		const transaction = makeTransaction(99999n);
		render(<SafeTxHeader safeTxHash={SAFE_TX_HASH} transaction={transaction} fromSafeApi={false} />);
		expect(screen.getByText(/99999/)).toBeTruthy();
	});
});
