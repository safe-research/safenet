import { type Address, getAbiItem, type Hex, numberToHex, type PublicClient, parseAbi, parseEventLogs } from "viem";
import z from "zod";
import type { SafeTransaction } from "./safe/service";
import { bigIntSchema, bytes32Schema, checkedAddressSchema, hexDataSchema } from "./schemas";
import { jsonReplacer } from "./utils";

const consensusAbi = parseAbi([
	"function getActiveEpoch() external view returns (uint64 epoch, bytes32 group)",
	"function proposeTransaction((uint256 chainId, address account, address to, uint256 value, uint8 operation, bytes data, uint256 nonce) transaction) external",
	"function getAttestation(uint64 epoch, (uint256 chainId, address account, address to, uint256 value, uint8 operation, bytes data, uint256 nonce) transaction) external view returns (bytes32 message, ((uint256 x, uint256 y) r, uint256 z) signature)",
	"function getAttestationByMessage(bytes32 message) external view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
	"struct MetaTransaction { uint256 chainId; address account; address to; uint256 value; uint8 operation; bytes data; uint256 nonce; }",
	"event TransactionProposed(bytes32 indexed message, bytes32 indexed transactionHash, uint64 epoch, MetaTransaction transaction)",
	"event TransactionAttested(bytes32 indexed message)",
]);

export type ConsensusState = {
	currentEpoch: bigint;
	currentGroupId: Hex;
	currentBlock: bigint;
};

export const loadConsensusState = async (provider: PublicClient, consensus: Address): Promise<ConsensusState> => {
	const currentBlock = await provider.getBlockNumber();
	const [epoch, groupId] = await provider.readContract({
		address: consensus,
		abi: consensusAbi,
		functionName: "getActiveEpoch",
	});
	return {
		currentEpoch: epoch,
		currentGroupId: groupId,
		currentBlock,
	};
};

export const transactionSchema = z.object({
	to: checkedAddressSchema,
	value: bigIntSchema,
	data: hexDataSchema,
	operation: z.union([z.literal(0), z.literal(1)]),
	nonce: bigIntSchema,
	chainId: bigIntSchema,
	account: checkedAddressSchema,
});

export const transactionProposedEventSchema = z.object({
	message: bytes32Schema,
	transactionHash: bytes32Schema,
	epoch: bigIntSchema,
	transaction: transactionSchema,
	proposedAt: bigIntSchema,
});

export type MetaTransaction = z.output<typeof transactionSchema>;

export type TransactionProposal = z.output<typeof transactionProposedEventSchema>;

const MAX_BLOCKS_RANGE = 50000n;

export const loadMessagesForTransaction = async (
	provider: PublicClient,
	consensus: Address,
	proposalTxHash: Hex,
): Promise<TransactionProposal[]> => {
	const blockNo = await provider.getBlockNumber();
	const logs = await provider.getLogs({
		address: consensus,
		event: getAbiItem({
			abi: consensusAbi,
			name: "TransactionProposed",
		}),
		args: {
			transactionHash: proposalTxHash,
		},
		fromBlock: blockNo - MAX_BLOCKS_RANGE,
	});
	return logs
		.sort((left, right) => {
			if (left.blockNumber !== right.blockNumber) {
				return left.blockNumber < right.blockNumber ? 1 : -1;
			}
			return right.logIndex - left.logIndex;
		})
		.map((log) => {
			const event = transactionProposedEventSchema.safeParse({
				...log.args,
				proposedAt: log.blockNumber,
			});
			return event.success ? event.data : undefined;
		})
		.filter((entry) => entry !== undefined);
};

export const loadRecentTransactionProposals = async (
	provider: PublicClient,
	consensus: Address,
): Promise<TransactionProposal[]> => {
	const blockNo = await provider.getBlockNumber();
	const logs = await provider.getLogs({
		address: consensus,
		event: getAbiItem({
			abi: consensusAbi,
			name: "TransactionProposed",
		}),
		fromBlock: blockNo - MAX_BLOCKS_RANGE,
	});
	return logs
		.sort((left, right) => {
			if (left.blockNumber !== right.blockNumber) {
				return left.blockNumber < right.blockNumber ? 1 : -1;
			}
			return right.logIndex - left.logIndex;
		})
		.map((log) => {
			const event = transactionProposedEventSchema.safeParse({
				...log.args,
				proposedAt: log.blockNumber,
			});
			return event.success ? event.data : undefined;
		})
		.filter((entry) => entry !== undefined);
};

export type TransactionDetails = {
	proposal: TransactionProposal;
	attestedAt: bigint | null;
};

export const loadTransactionProposalDetails = async (
	provider: PublicClient,
	consensus: Address,
	message: Hex,
): Promise<TransactionDetails | null> => {
	const blockNo = await provider.getBlockNumber();
	const logs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: consensus,
				topics: [null, message],
				fromBlock: numberToHex(blockNo - MAX_BLOCKS_RANGE),
			},
		],
	});
	const events = parseEventLogs({
		logs,
		abi: consensusAbi,
	});
	const proposalEvent = events.find((e) => e.eventName === "TransactionProposed");
	if (proposalEvent === undefined) return null;
	const parsedProposal = transactionProposedEventSchema.safeParse({
		...proposalEvent.args,
		proposedAt: proposalEvent.blockNumber,
	});
	const attestationEvent = events.find((e) => e.eventName === "TransactionAttested");
	return parsedProposal.success
		? {
				proposal: parsedProposal.data,
				attestedAt: attestationEvent?.blockNumber !== undefined ? BigInt(attestationEvent.blockNumber) : null,
			}
		: null;
};

export const postTransactionProposal = async (url: string, transaction: SafeTransaction) => {
	const response = await fetch(url, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(transaction, jsonReplacer),
	});

	if (!response.ok) throw new Error("Network response was not ok");
};
