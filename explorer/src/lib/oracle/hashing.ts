import { type Address, type Hex, hashTypedData } from "viem";

export type OracleRequestIdParams = {
	chainId: number | bigint;
	consensus: Address;
	epoch: bigint;
	oracle: Address;
	safeTxHash: Hex;
};

// Mirrors `ConsensusMessages.transactionProposal` (validator's `transaction_proposal_hash`).
// The result isn't just an attestation message hash — `Consensus.proposeTransaction`
// posts this same value to the oracle as its `requestId`, so it doubles as the lookup key for
// `getRequest`/`Committed`/`Revealed`/`OracleResult`.
export const oracleRequestId = ({ chainId, consensus, epoch, oracle, safeTxHash }: OracleRequestIdParams): Hex =>
	hashTypedData({
		domain: {
			chainId,
			verifyingContract: consensus,
		},
		types: {
			TransactionProposal: [
				{ type: "uint64", name: "epoch" },
				{ type: "address", name: "oracle" },
				{ type: "bytes32", name: "safeTxHash" },
			],
		},
		primaryType: "TransactionProposal",
		message: { epoch, oracle, safeTxHash },
	});
