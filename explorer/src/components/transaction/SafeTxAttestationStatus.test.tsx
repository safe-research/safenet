// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import type { Address, Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SafeTransaction, TransactionProposal } from "@/lib/consensus";
import { SafeTxAttestationStatus } from "./SafeTxAttestationStatus";

vi.mock("@/hooks/useValidatorInfo", () => ({
	useValidatorInfoMap: vi.fn(() => ({ data: null })),
}));

vi.mock("@/hooks/useSigningProgress", () => ({
	useAttestationStatus: vi.fn(() => ({ data: null, isFetching: false })),
}));

import { useAttestationStatus } from "@/hooks/useSigningProgress";
import { useValidatorInfoMap } from "@/hooks/useValidatorInfo";

afterEach(cleanup);

const VALIDATOR_A = "0x00000000000000000000000000000000000000A1" as Address;
const VALIDATOR_B = "0x00000000000000000000000000000000000000B2" as Address;

const SAFE_TX_HASH = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" as Hex;

const transaction: SafeTransaction = {
	chainId: 100n,
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
};

const proposal: TransactionProposal = {
	chainId: 100n,
	safeTxHash: SAFE_TX_HASH,
	epoch: 1n,
	oracle: "0x0000000000000000000000000000000000000099" as Address,
	oracleData: "0x" as Hex,
	requestId: `0x${"cd".repeat(32)}` as Hex,
	transaction,
	proposedAt: { block: 100n, tx: "0xabc" as Hex },
	attestedAt: null,
};

// The text of the row with the given label, so assertions see the label and its value together.
const rowText = (label: string) => screen.getByText(label).parentElement?.textContent;

describe("SafeTxAttestationStatus", () => {
	it("marks a validator that has not committed to a pending round as pending, not missed", () => {
		vi.mocked(useValidatorInfoMap).mockReturnValue({
			data: new Map([
				[VALIDATOR_A, { address: VALIDATOR_A, label: "alice" }],
				[VALIDATOR_B, { address: VALIDATOR_B, label: "bob" }],
			]),
		} as never);
		vi.mocked(useAttestationStatus).mockReturnValue({
			isFetching: false,
			data: {
				status: "pending",
				sid: `0x${"aa".repeat(32)}`,
				groupId: `0x${"00".repeat(32)}`,
				sequence: 0n,
				lastUpdate: 102n,
				committed: [{ address: VALIDATOR_A, block: 102n }],
				signed: [],
			},
		} as never);
		render(<SafeTxAttestationStatus proposal={proposal} />);
		expect(rowText("Committed:")).toBe("Committed:alice ✅, bob ⏳");
	});
});
