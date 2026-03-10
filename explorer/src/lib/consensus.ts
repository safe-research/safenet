import {
	type Address,
	formatLog,
	getAbiItem,
	type Hex,
	numberToHex,
	type PublicClient,
	parseAbi,
	parseEventLogs,
	toEventSelector,
} from "viem";
import z from "zod";
import { bigIntSchema, checkedAddressSchema, hexDataSchema } from "@/lib/schemas";
import { getBlockRange, jsonReplacer, mostRecentFirst } from "@/lib/utils";

const consensusAbi = parseAbi([
	"function getActiveEpoch() external view returns (uint64 epoch, bytes32 group)",
	"function getEpochsState() external view returns (uint64 previous, uint64 active, uint64 staged, uint64 rolloverBlock)",
	"function getEpochGroupId(uint64 epoch) external view returns (bytes32 group)",
	"function proposeTransaction((uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction) external returns (bytes32 transactionHash)",
	"function getTransactionAttestationByHash(uint64 epoch, bytes32 transactionHash) external view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
	"event TransactionProposed(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction)",
	"event TransactionAttested(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, bytes32 signatureId, ((uint256 x, uint256 y) r, uint256 z) attestation)",
	"event EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256 x, uint256 y) groupKey)",
	"event EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256 x, uint256 y) groupKey, bytes32 signatureId, ((uint256 x, uint256 y) r, uint256 z) attestation)",
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

export type ExecutionLink = {
	block: bigint;
	tx: Hex;
};

export type TransactionProposal = {
	chainId: bigint;
	safeTxHash: Hex;
	epoch: bigint;
	transaction: SafeTransaction;
	proposedAt: ExecutionLink;
	attestedAt: ExecutionLink | null;
};

const [proposedEventSelector, attestedEventSelector] = [
	"TransactionProposed" as const,
	"TransactionAttested" as const,
].map((eventName) => toEventSelector(getAbiItem({ abi: consensusAbi, name: eventName })));
const transactionEventSelectors = [proposedEventSelector, attestedEventSelector];

export const loadProposedSafeTransaction = async ({
	provider,
	consensus,
	safeTxHash,
	maxBlockRange,
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash: Hex;
	maxBlockRange: bigint;
}): Promise<SafeTransaction | null> => {
	const { fromBlock, toBlock } = await getBlockRange(provider, maxBlockRange);
	const logs = await provider.getLogs({
		address: consensus,
		event: getAbiItem({
			abi: consensusAbi,
			name: "TransactionProposed",
		}),
		args: {
			safeTxHash,
		},
		fromBlock,
		toBlock,
		strict: true,
	});
	return safeTransactionSchema.safeParse(logs.at(0)?.args?.transaction).data ?? null;
};

export type LoadTransactionProposalsResult = {
	proposals: TransactionProposal[];
	fromBlock: bigint;
	toBlock: bigint;
};

export const loadTransactionProposals = async ({
	provider,
	consensus,
	safeTxHash,
	safe,
	toBlock: referenceBlock,
	maxBlockRange,
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash?: Hex;
	safe?: Address;
	toBlock?: bigint;
	maxBlockRange: bigint;
}): Promise<LoadTransactionProposalsResult> => {
	const { fromBlock, toBlock } = await getBlockRange(provider, maxBlockRange, referenceBlock);
	const blockRange = { fromBlock: numberToHex(fromBlock), toBlock: numberToHex(toBlock) };

	// We use an `eth_getLogs` here directly, in order to filter on the `safeTxHash` topic.
	// When `safe` is set, topic[3] silently drops `TransactionAttested` (only 1 indexed topic);
	// those proposals will have attestedAt: null until contract events are updated.
	const rawLogs = await provider.request({

		method: "eth_getLogs",
		params: [
			{
				address: consensus,
				...blockRange,
				topics: [transactionEventSelectors, safeTxHash ?? null, null, safe ?? null],
			},
		],
	});
	const eventLogs = mostRecentFirst(
		parseEventLogs({
			// <https://github.com/wevm/viem/issues/4340>
			logs: rawLogs.map((log) => formatLog(log)),
			abi: consensusAbi,
			strict: true,
		}),
	);

	const attestationKey = (log: { args: { safeTxHash: Hex; epoch: bigint } }) =>
		`${log.args.safeTxHash}:${log.args.epoch}`;
	const attestations = new Map(
		eventLogs
			.filter((log) => log.eventName === "TransactionAttested")
			.map((log) => [attestationKey(log), { block: log.blockNumber, tx: log.transactionHash }] as const),
	);
	const proposals = eventLogs
		.map((log) => {
			if (log.eventName !== "TransactionProposed") {
				return undefined;
			}

			const transaction = safeTransactionSchema.safeParse(log.args.transaction);
			if (!transaction.success) {
				return undefined;
			}

			const attestation = attestations.get(attestationKey(log));
			return {
				chainId: log.args.chainId,
				safeTxHash: log.args.safeTxHash,
				epoch: log.args.epoch,
				transaction: transaction.data,
				proposedAt: {
					block: log.blockNumber,
					tx: log.transactionHash,
				},
				attestedAt: attestation ?? null,
			};
		})
		.filter((proposal) => proposal !== undefined);

	return { proposals, fromBlock, toBlock };
};

export type EpochsState = {
	previous: bigint;
	active: bigint;
	staged: bigint;
	rolloverBlock: bigint;
	activeGroupId: Hex;
	stagedGroupId: Hex | null;
};

export type EpochRolloverEntry = {
	activeEpoch: bigint;
	proposedEpoch: bigint;
	rolloverBlock: bigint;
	groupId: Hex;
	stagedAt: bigint;
};

export const loadEpochsState = async (provider: PublicClient, consensus: Address): Promise<EpochsState> => {
	const [previous, active, staged, rolloverBlock] = await provider.readContract({
		address: consensus,
		abi: consensusAbi,
		functionName: "getEpochsState",
	});
	const [activeGroupId, stagedGroupId] = await Promise.all([
		provider.readContract({
			address: consensus,
			abi: consensusAbi,
			functionName: "getEpochGroupId",
			args: [active],
		}),
		staged > 0n
			? provider.readContract({
					address: consensus,
					abi: consensusAbi,
					functionName: "getEpochGroupId",
					args: [staged],
				})
			: Promise.resolve(null),
	]);
	return { previous, active, staged, rolloverBlock, activeGroupId, stagedGroupId };
};

export type EpochRolloverResult = {
	entries: EpochRolloverEntry[];
	reachedGenesis: boolean;
	fromBlock: bigint;
};

export const loadEpochRolloverHistory = async ({
	provider,
	consensus,
	maxBlockRange,
	cursor,
}: {
	provider: PublicClient;
	consensus: Address;
	maxBlockRange: bigint;
	cursor?: bigint;
}): Promise<EpochRolloverResult> => {
	// When a cursor is provided, subtract 1 so the event at the cursor block (already
	// included in the previous page) is not fetched again.
	const referenceBlock = cursor !== undefined ? cursor - 1n : undefined;
	const { fromBlock, toBlock } = await getBlockRange(provider, maxBlockRange, referenceBlock);

	const stagedLogs = mostRecentFirst(
		await provider.getLogs({
			address: consensus,
			event: getAbiItem({ abi: consensusAbi, name: "EpochStaged" }),
			fromBlock,
			toBlock,
			strict: true,
		}),
	);

	const entries: EpochRolloverEntry[] = stagedLogs.map((log) => ({
		activeEpoch: log.args.activeEpoch,
		proposedEpoch: log.args.proposedEpoch,
		rolloverBlock: log.args.rolloverBlock,
		groupId: log.args.groupId,
		stagedAt: log.blockNumber,
	}));

	// Reached genesis when an activeEpoch 0 entry is found, or when the search window
	// has reached block 0 (nothing further to search).
	const reachedGenesis = fromBlock === 0n || entries.some((e) => e.activeEpoch === 0n);
	return {
		entries,
		reachedGenesis,
		fromBlock,
	};
};

export const postTransactionProposal = async (url: string, transaction: SafeTransaction) => {
	const response = await fetch(url, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(transaction, jsonReplacer),
	});

	if (!response.ok) throw new Error("Network response was not ok");
};
