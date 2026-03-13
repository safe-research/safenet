// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Address, Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SafeTransaction } from "@/lib/consensus";
import { SafeTxSummary } from "./SafeTxSummary";

vi.mock("@/components/common/CopyButton", () => ({
	CopyButton: ({ value }: { value: string }) => (
		<button type="button" data-testid="copy-button">
			{value}
		</button>
	),
}));

vi.mock("@/components/common/InlineAddress", () => ({
	InlineAddress: ({ address }: { address: string }) => <span data-testid="inline-address">{address}</span>,
}));

vi.mock("@/hooks/useSettings", () => ({
	useSettings: vi.fn(() => [
		{
			decoder: "https://calldata.swiss-knife.xyz/decoder?calldata=",
			rpc: "https://rpc.example.com",
			consensus: "0x0000000000000000000000000000000000000001" as Address,
			maxBlockRange: 10000,
			validatorInfo: "https://example.com/validators.json",
			refetchInterval: 10000,
			blocksPerEpoch: 1440,
		},
	]),
}));

afterEach(cleanup);

const TO_ADDRESS = "0xBEEFc0ffee0000000000000000000000000000c0" as Address;

const makeTransaction = (overrides?: Partial<SafeTransaction>): SafeTransaction => ({
	chainId: 1n,
	safe: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as Address,
	to: TO_ADDRESS,
	value: 120000000000000000n, // 0.12 ETH
	data: "0xa9059cbb" as Hex,
	operation: 0,
	safeTxGas: 0n,
	baseGas: 0n,
	gasPrice: 0n,
	gasToken: "0x0000000000000000000000000000000000000000" as Address,
	refundReceiver: "0x0000000000000000000000000000000000000000" as Address,
	nonce: 0n,
	...overrides,
});

describe("SafeTxSummary", () => {
	it("renders the Transaction Summary title", () => {
		render(<SafeTxSummary transaction={makeTransaction()} />);
		expect(screen.getByText("Transaction Summary")).toBeTruthy();
	});

	it("renders the Operation field as CALL for operation=0", () => {
		render(<SafeTxSummary transaction={makeTransaction({ operation: 0 })} />);
		expect(screen.getByText("CALL")).toBeTruthy();
	});

	it("renders the Operation field as DELEGATECALL for operation=1", () => {
		render(<SafeTxSummary transaction={makeTransaction({ operation: 1 })} />);
		expect(screen.getByText("DELEGATECALL")).toBeTruthy();
	});

	it("renders the To address via InlineAddress", () => {
		render(<SafeTxSummary transaction={makeTransaction()} />);
		const addressEl = screen.getByTestId("inline-address");
		expect(addressEl.textContent).toBe(TO_ADDRESS);
	});

	it("renders a CopyButton for the To address", () => {
		render(<SafeTxSummary transaction={makeTransaction()} />);
		const copyButtons = screen.getAllByTestId("copy-button");
		const toAddressCopy = copyButtons.find((btn) => btn.textContent === TO_ADDRESS);
		expect(toAddressCopy).toBeTruthy();
	});

	it("renders the Value field", () => {
		render(<SafeTxSummary transaction={makeTransaction()} />);
		expect(screen.getByText(/0.12/)).toBeTruthy();
		expect(screen.getByText(/ETH/)).toBeTruthy();
	});

	it("renders the Calldata field with data size", () => {
		render(<SafeTxSummary transaction={makeTransaction()} />);
		// dataString returns "X bytes of data" for "0xa9059cbb" (4 bytes)
		expect(screen.getByText(/bytes/)).toBeTruthy();
	});

	it("renders the Raw calldata label", () => {
		render(<SafeTxSummary transaction={makeTransaction()} />);
		expect(screen.getByText("Raw calldata:")).toBeTruthy();
	});

	it("renders a CopyButton for the raw calldata", () => {
		const data = "0xa9059cbb" as Hex;
		render(<SafeTxSummary transaction={makeTransaction({ data })} />);
		const copyButtons = screen.getAllByTestId("copy-button");
		const calldataCopy = copyButtons.find((btn) => btn.textContent === data);
		expect(calldataCopy).toBeTruthy();
	});

	it("renders the Decode link with the correct href", () => {
		const data = "0xa9059cbb" as Hex;
		render(<SafeTxSummary transaction={makeTransaction({ data })} />);
		const decodeLink = screen.getByRole("link", { name: /decode/i });
		expect(decodeLink.getAttribute("href")).toBe(`https://calldata.swiss-knife.xyz/decoder?calldata=${data}`);
	});

	it("does not show more/less toggle for short calldata", () => {
		render(<SafeTxSummary transaction={makeTransaction({ data: "0xa9059cbb" as Hex })} />);
		expect(screen.queryByRole("button", { name: /more/i })).toBeNull();
		expect(screen.queryByRole("button", { name: /less/i })).toBeNull();
	});

	it("shows more/less toggle for long calldata and toggles it", () => {
		// Generate calldata longer than 206 chars: "0x" + 104 bytes = "0x" + 208 hex chars = 210 chars total
		const longData = `0x${"a9".repeat(104)}` as Hex;
		render(<SafeTxSummary transaction={makeTransaction({ data: longData })} />);
		const moreButton = screen.getByRole("button", { name: /more/i });
		expect(moreButton).toBeTruthy();
		fireEvent.click(moreButton);
		expect(screen.getByRole("button", { name: /less/i })).toBeTruthy();
	});
});
