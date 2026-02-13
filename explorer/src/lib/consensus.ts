import {
	type AbiEvent,
	type Address,
	getAbiItem,
	type Hex,
	type Log,
	numberToHex,
	type PublicClient,
	parseAbi,
	parseEventLogs,
} from "viem";
import z from "zod";
import { bigIntSchema, bytes32Schema, checkedAddressSchema, hexDataSchema } from "./schemas";
import { jsonReplacer } from "./utils";

const consensusAbi = parseAbi([
	"function getActiveEpoch() external view returns (uint64 epoch, bytes32 group)",
	"function proposeTransaction((uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction) external returns (bytes32 transactionHash)",
	"function getTransactionAttestationByHash(uint64 epoch, bytes32 transactionHash) external view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
	"event TransactionProposed(bytes32 indexed transactionHash, uint256 indexed chainId, address indexed safe, uint64 epoch, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction)",
	"event TransactionAttested(bytes32 indexed transactionHash, uint64 epoch, ((uint256 x, uint256 y) r, uint256 z) attestation)",
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

export const safeTransactionSchema = z.object({
	chainId: bigIntSchema,
	safe: checkedAddressSchema,
	to: checkedAddressSchema,
	value: bigIntSchema,
	data: hexDataSchema,
	operation: z.union([z.literal(0), z.literal(1)]),
	safeTxGas: bigIntSchema,
	baseGas: bigIntSchema,
	gasPrice: bigIntSchema,
	gasToken: checkedAddressSchema,
	refundReceiver: checkedAddressSchema,
	nonce: bigIntSchema,
});

export const transactionProposedSchema = z.object({
	proposedAt: bigIntSchema,
	transactionHash: bytes32Schema,
	chainId: bigIntSchema,
	safe: checkedAddressSchema,
	epoch: bigIntSchema,
	transaction: safeTransactionSchema,
});

export type SafeTransaction = z.output<typeof safeTransactionSchema>;

export type TransactionProposal = z.output<typeof transactionProposedSchema>;

const MAX_BLOCKS_RANGE = 50000n;

const getFromBlock = async (provider: PublicClient): Promise<bigint> => {
	const blockNo = await provider.getBlockNumber();
	return blockNo > MAX_BLOCKS_RANGE ? blockNo - MAX_BLOCKS_RANGE : 0n;
};

const transactionProposedEvent = getAbiItem({
	abi: consensusAbi,
	name: "TransactionProposed",
});

const mostRecentFirst = <T extends Pick<Log<bigint, number, false>, "blockNumber" | "logIndex">>(logs: T[]): T[] =>
	logs.sort((left, right) => {
		if (left.blockNumber !== right.blockNumber) {
			return left.blockNumber < right.blockNumber ? 1 : -1;
		}
		return right.logIndex - left.logIndex;
	});

const parseTransactionProposal = <E extends AbiEvent>(
	log: Log<bigint, number, false, E> | null | undefined,
): TransactionProposal | undefined =>
	transactionProposedSchema.safeParse({
		...log?.args,
		proposedAt: log?.blockNumber,
	}).data;

export const loadProposalsForTransaction = async (
	provider: PublicClient,
	consensus: Address,
	proposalTxHash: Hex,
): Promise<TransactionProposal[]> => {
	const fromBlock = await getFromBlock(provider);
	const logs = await provider.getLogs({
		address: consensus,
		event: transactionProposedEvent,
		args: {
			transactionHash: proposalTxHash,
		},
		fromBlock,
	});
	return mostRecentFirst(logs)
		.map(parseTransactionProposal)
		.filter((e) => e !== undefined);
};

export const loadRecentTransactionProposals = async (
	provider: PublicClient,
	consensus: Address,
): Promise<TransactionProposal[]> => {
	const fromBlock = await getFromBlock(provider);
	const logs = await provider.getLogs({
		address: consensus,
		event: transactionProposedEvent,
		fromBlock,
	});
	return mostRecentFirst(logs)
		.map(parseTransactionProposal)
		.filter((e) => e !== undefined);
};

export type TransactionDetails = {
	proposal: TransactionProposal;
	attestedAt: bigint | null;
};

export const loadTransactionProposalDetails = async (
	provider: PublicClient,
	consensus: Address,
	safeTxHash: Hex,
): Promise<TransactionDetails | null> => {
	const fromBlock = await getFromBlock(provider);
	const logs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: consensus,
				topics: [null, safeTxHash],
				fromBlock: numberToHex(fromBlock),
			},
		],
	});
	const events = mostRecentFirst(
		parseEventLogs({
			logs,
			abi: consensusAbi,
			strict: true,
		}),
	);

	// First look for attestations, and try to return transaction details for it.
	for (const attestation of events.filter((e) => e.eventName === "TransactionAttested")) {
		const proposal = parseTransactionProposal(
			events.find((e) => e.eventName === "TransactionProposed" && e.args.epoch === attestation.args.epoch),
		);
		if (proposal === undefined) {
			continue;
		}

		return {
			proposal,
			attestedAt: attestation.blockNumber,
		};
	}

	// If we can't find an attestation with a matching transaction proposal, return the most recent pending proposal.
	const proposal = parseTransactionProposal(events.find((e) => e.eventName === "TransactionProposed"));
	if (proposal !== undefined) {
		return {
			proposal,
			attestedAt: null,
		};
	}

	// We can't find anything...
	return null;
};

export const postTransactionProposal = async (url: string, transaction: SafeTransaction) => {
	const response = await fetch(url, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(transaction, jsonReplacer),
	});

	if (!response.ok) throw new Error("Network response was not ok");
};
