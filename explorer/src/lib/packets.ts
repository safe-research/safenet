import { type Hex, hashTypedData, type Prettify, type TypedDataDomain } from "viem";

export type SafeTransactionProposal = {
	domain: Prettify<Required<Pick<TypedDataDomain, "chainId" | "verifyingContract">>>;
	proposal: {
		epoch: bigint;
		safeTxHash: Hex;
	};
};

export const safeTxProposalHash = ({ domain, proposal }: SafeTransactionProposal) =>
	hashTypedData({
		domain,
		types: {
			TransactionProposal: [
				{ type: "uint64", name: "epoch" },
				{ type: "bytes32", name: "safeTxHash" },
			],
		},
		primaryType: "TransactionProposal",
		message: proposal,
	});
