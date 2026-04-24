import { type Address, type Hex, hashTypedData } from "viem";
import { safeTxHash } from "../safeTx/hashing.js";
import type { OracleTransactionPacket } from "./schemas.js";

export type OracleTransactionProposal = {
	domain: OracleTransactionPacket["domain"];
	proposal: {
		epoch: bigint;
		oracle: Address;
		safeTxHash: Hex;
	};
};

export const oracleTxProposalHash = ({ domain, proposal }: OracleTransactionProposal): Hex =>
	hashTypedData({
		domain: {
			chainId: domain.chain,
			verifyingContract: domain.consensus,
		},
		types: {
			OracleTransactionProposal: [
				{ type: "uint64", name: "epoch" },
				{ type: "address", name: "oracle" },
				{ type: "bytes32", name: "safeTxHash" },
			],
		},
		primaryType: "OracleTransactionProposal",
		message: proposal,
	});

export const oracleTxPacketHash = (packet: OracleTransactionPacket): Hex =>
	oracleTxProposalHash({
		domain: packet.domain,
		proposal: {
			epoch: packet.proposal.epoch,
			oracle: packet.proposal.oracle,
			safeTxHash: safeTxHash(packet.proposal.transaction),
		},
	});
