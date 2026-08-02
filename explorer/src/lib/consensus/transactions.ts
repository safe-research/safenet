import {
	type Address,
	formatLog,
	getAbiItem,
	getAddress,
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
	oracle: Address | null;
	transaction: SafeTransaction;
	proposedAt: ExecutionLink;
	attestedAt: ExecutionLink | null;
};

export type ProposalStatus = "ATTESTED" | "PROPOSED" | "TIMED_OUT";

export type TransactionProposalWithStatus = TransactionProposal & { status: ProposalStatus };

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
	oracles = [],
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash?: Hex;
	safe?: Address;
	toBlock?: bigint;
	maxBlockRange: bigint;
	signingTimeout: number;
	oracles?: Address[];
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
	const allEventLogs = mostRecentFirst(
		parseEventLogs({
			// <https://github.com/wevm/viem/issues/4340>
			logs: rawLogs.map((log) => formatLog(log)),
			abi: consensusAbi,
			strict: true,
		}),
	);

	// `oracle` isn't an indexed topic on either oracle event, so it can't be filtered via
	// eth_getLogs topics; trust is resolved here, after decoding. With no explicit allow-list,
	// an oracle is trusted once it has an `OracleTransactionAttested` log in this same batch.
	// Addresses are compared via their checksummed form: `oracles` comes from user settings and
	// may not be checksummed, while addresses decoded from logs by viem always are.
	const trustedOracles = new Set(
		(oracles.length > 0
			? oracles
			: allEventLogs.filter((log) => log.eventName === "OracleTransactionAttested").map((log) => log.args.oracle)
		).map((oracle) => getAddress(oracle)),
	);
	const eventLogs = allEventLogs.filter((log) => {
		if (log.eventName !== "OracleTransactionProposed" && log.eventName !== "OracleTransactionAttested") {
			return true;
		}
		return trustedOracles.has(getAddress(log.args.oracle));
	});

	const attestationKey = (log: { args: { safeTxHash: Hex; epoch: bigint; oracle?: Address } }) =>
		`${log.args.safeTxHash}:${log.args.epoch}:${log.args.oracle ? getAddress(log.args.oracle) : "plain"}`;
	const attestations = new Map(
		eventLogs
			.filter((log) => log.eventName === "TransactionAttested" || log.eventName === "OracleTransactionAttested")
			.map((log) => [attestationKey(log), { block: log.blockNumber, tx: log.transactionHash }] as const),
	);
	const proposals = eventLogs
		.map((log) => {
			if (log.eventName !== "TransactionProposed" && log.eventName !== "OracleTransactionProposed") {
				return undefined;
			}

			const transaction = safeTransactionSchema.safeParse(log.args.transaction);
			if (!transaction.success) {
				return undefined;
			}

			const oracle = log.eventName === "OracleTransactionProposed" ? log.args.oracle : null;
			const attestedAt = attestations.get(attestationKey(log)) ?? null;
			const proposedAt = { block: log.blockNumber, tx: log.transactionHash };
			const status: ProposalStatus =
				attestedAt !== null
					? "ATTESTED"
					: toBlock - proposedAt.block > BigInt(signingTimeout)
						? "TIMED_OUT"
						: "PROPOSED";
			return {
				chainId: log.args.chainId,
				safeTxHash: log.args.safeTxHash,
				epoch: log.args.epoch,
				oracle,
				transaction: transaction.data,
				proposedAt,
				attestedAt,
				status,
			};
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
