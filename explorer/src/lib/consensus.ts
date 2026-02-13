import {
	type Address,
	formatLog,
	getAbiItem,
	type Hex,
	type Log,
	numberToHex,
	type PublicClient,
	parseAbi,
	parseEventLogs,
	toEventSelector,
} from "viem";
import z from "zod";
import { bigIntSchema, checkedAddressSchema, hexDataSchema } from "@/lib/schemas";
import { jsonReplacer } from "@/lib/utils";

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

export type SafeTransaction = z.output<typeof safeTransactionSchema>;

export type TransactionProposal = {
	safeTxHash: Hex;
	epoch: bigint;
	transaction: SafeTransaction;
	proposedAt: bigint;
	attestedAt: bigint | null;
};

const MAX_BLOCKS_RANGE = 50000n;

const getFromBlock = async (provider: PublicClient): Promise<bigint> => {
	const blockNumber = await provider.getBlockNumber();
	return blockNumber > MAX_BLOCKS_RANGE ? blockNumber - MAX_BLOCKS_RANGE : 0n;
};

const mostRecentFirst = <T extends Pick<Log<bigint, number, false>, "blockNumber" | "logIndex">>(logs: T[]): T[] =>
	logs.sort((left, right) => {
		if (left.blockNumber !== right.blockNumber) {
			return left.blockNumber < right.blockNumber ? 1 : -1;
		}
		return right.logIndex - left.logIndex;
	});

const transactionEventSelectors = ["TransactionProposed" as const, "TransactionAttested" as const].map((eventName) =>
	toEventSelector(
		getAbiItem({
			abi: consensusAbi,
			name: eventName,
		}),
	),
);

export const loadProposedSafeTransaction = async ({
	provider,
	consensus,
	safeTxHash,
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash: Hex;
}): Promise<SafeTransaction | null> => {
	const fromBlock = await getFromBlock(provider);
	const logs = await provider.getLogs({
		address: consensus,
		event: getAbiItem({
			abi: consensusAbi,
			name: "TransactionProposed",
		}),
		args: {
			transactionHash: safeTxHash,
		},
		fromBlock,
		strict: true,
	});
	return safeTransactionSchema.safeParse(logs.at(0)?.args?.transaction).data ?? null;
};

export const loadTransactionProposals = async ({
	provider,
	consensus,
	safeTxHash,
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash?: Hex;
}): Promise<TransactionProposal[]> => {
	// We use an `eth_getLogs` here directly, in order to filter on the `transactionHash` of both `TransactionProposed`
	// and `TransactionAttested` events.
	const fromBlock = await getFromBlock(provider);
	const logs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: consensus,
				topics: [transactionEventSelectors, safeTxHash ?? null],
				fromBlock: numberToHex(fromBlock),
			},
		],
	});
	const eventLogs = mostRecentFirst(
		parseEventLogs({
			// <https://github.com/wevm/viem/issues/4340>
			logs: logs.map((log) => formatLog(log)),
			abi: consensusAbi,
			strict: true,
		}),
	);

	const attestationKey = (log: { args: { transactionHash: Hex; epoch: bigint } }) =>
		`${log.args.transactionHash}:${log.args.epoch}`;
	const attestations = new Map(
		eventLogs
			.filter((log) => log.eventName === "TransactionAttested")
			.map((log) => [attestationKey(log), log.blockNumber] as const),
	);
	return eventLogs
		.map((log) => {
			if (log.eventName !== "TransactionProposed") {
				return undefined;
			}

			const transaction = safeTransactionSchema.safeParse(log.args.transaction);
			if (!transaction.success) {
				return undefined;
			}

			return {
				safeTxHash: log.args.transactionHash,
				epoch: log.args.epoch,
				transaction: transaction.data,
				proposedAt: log.blockNumber,
				attestedAt: attestations.get(attestationKey(log)) ?? null,
			};
		})
		.filter((proposal) => proposal !== undefined);
};

export const postTransactionProposal = async (url: string, transaction: SafeTransaction) => {
	const response = await fetch(url, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(transaction, jsonReplacer),
	});

	if (!response.ok) throw new Error("Network response was not ok");
};
