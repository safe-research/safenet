import {
	type Address,
	formatLog,
	getAbiItem,
	type Hex,
	numberToHex,
	type PublicClient,
	pad,
	parseEventLogs,
} from "viem";
import z from "zod";
import { bigIntSchema, checkedAddressSchema, hexDataSchema } from "@/lib/schemas";
import { getBlockRange, jsonReplacer, mostRecentFirst } from "@/lib/utils";
import { consensusAbi, transactionEventSelectors } from "./abi";

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

export type ProposalStatus = "ATTESTED" | "PROPOSED" | "TIMED_OUT";

export type TransactionProposalWithStatus = TransactionProposal & { status: ProposalStatus };

export function getProposalStatus(
	proposal: TransactionProposal,
	currentBlock: bigint,
	signingTimeout: number,
): ProposalStatus {
	if (proposal.attestedAt !== null) return "ATTESTED";
	if (currentBlock - proposal.proposedAt.block > BigInt(signingTimeout)) return "TIMED_OUT";
	return "PROPOSED";
}

export type LoadTransactionProposalsResult = {
	proposals: TransactionProposalWithStatus[];
	fromBlock: bigint;
	toBlock: bigint;
};

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

export const loadTransactionProposals = async ({
	provider,
	consensus,
	safeTxHash,
	safe,
	toBlock: referenceBlock,
	maxBlockRange,
	signingTimeout,
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash?: Hex;
	safe?: Address;
	toBlock?: bigint;
	maxBlockRange: bigint;
	signingTimeout: number;
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
				topics: [transactionEventSelectors, safeTxHash ?? null, null, safe ? pad(safe) : null],
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
			const proposal: TransactionProposal = {
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
			return { ...proposal, status: getProposalStatus(proposal, toBlock, signingTimeout) };
		})
		.filter((proposal) => proposal !== undefined);

	return { proposals, fromBlock, toBlock };
};

export const postTransactionProposal = async (url: string, transaction: SafeTransaction) => {
	const response = await fetch(url, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(transaction, jsonReplacer),
	});

	if (!response.ok) throw new Error("Network response was not ok");
};
