import { type Address, type Hex, hashTypedData } from "viem";

export type OracleRequestIdParams = {
	chainId: number | bigint;
	consensus: Address;
	epoch: bigint;
	oracle: Address;
	safeTxHash: Hex;
};

// Mirrors `ConsensusMessages.oracleTransactionProposal` (validator's `oracleTxProposalHash`).
// The result isn't just an attestation message hash — `Consensus.proposeOracleTransaction`
// posts this same value to the oracle as its `requestId`, so it doubles as the lookup key for
// `getRequest`/`Committed`/`Revealed`/`OracleResult`.
export const oracleRequestId = ({ chainId, consensus, epoch, oracle, safeTxHash }: OracleRequestIdParams): Hex =>
	hashTypedData({
		domain: {
			chainId,
			verifyingContract: consensus,
		},
		types: {
			OracleTransactionProposal: [
				{ type: "uint64", name: "epoch" },
				{ type: "address", name: "oracle" },
				{ type: "bytes32", name: "safeTxHash" },
			],
		},
		primaryType: "OracleTransactionProposal",
		message: { epoch, oracle, safeTxHash },
	});
