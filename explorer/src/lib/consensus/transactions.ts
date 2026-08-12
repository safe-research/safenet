import {
	type Address,
	formatLog,
	getAddress,
	type Hex,
	numberToHex,
	type PublicClient,
	pad,
	parseEventLogs,
	toHex,
} from "viem";
import z from "zod";
import { bigIntSchema, checkedAddressSchema, hexDataSchema } from "@/lib/schemas";
import { getBlockRange, jsonReplacer, mostRecentFirst } from "@/lib/utils";
import { consensusAbi, proposedEventSelectors, transactionEventSelectors } from "./abi";

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
	oracle: Address;
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

// A Safe smart account on a specific chain: a Safe address is only unique per chain.
export type SafeId = { chainId: bigint; safe: Address };

// Mirrors `SafeId.create` in contracts/src/libraries/SafeId.sol: the chain ID occupies the upper
// 96 bits and the address the lower 160 bits of the resulting bytes32.
export const computeSafeId = ({ chainId, safe }: SafeId): Hex => toHex((chainId << 160n) | BigInt(safe), { size: 32 });

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
	const rawLogs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: consensus,
				fromBlock: numberToHex(fromBlock),
				toBlock: numberToHex(toBlock),
				topics: [proposedEventSelectors, safeTxHash],
			},
		],
	});
	const logs = parseEventLogs({
		logs: rawLogs.map((log) => formatLog(log)),
		abi: consensusAbi,
		eventName: "TransactionProposed",
		strict: true,
	});
	return safeTransactionSchema.safeParse(logs.at(0)?.args?.transaction).data ?? null;
};

export const loadTransactionProposals = async ({
	provider,
	consensus,
	safeTxHash,
	safeId,
	toBlock: referenceBlock,
	maxBlockRange,
	signingTimeout,
	oracles = [],
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash?: Hex;
	safeId?: SafeId;
	toBlock?: bigint;
	maxBlockRange: bigint;
	signingTimeout: number;
	oracles?: Address[];
}): Promise<LoadTransactionProposalsResult> => {
	const { fromBlock, toBlock } = await getBlockRange(provider, maxBlockRange, referenceBlock);
	const blockRange = { fromBlock: numberToHex(fromBlock), toBlock: numberToHex(toBlock) };

	// `TransactionProposed` and `TransactionAttested` both index `safeTxHash`, `safeId` and `oracle`,
	// so an explicit allow-list can be pushed straight into the `oracle` topic as an OR filter.
	const rawLogs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: consensus,
				...blockRange,
				topics: [
					transactionEventSelectors,
					safeTxHash ?? null,
					safeId ? computeSafeId(safeId) : null,
					oracles.length > 0 ? oracles.map((oracle) => pad(oracle)) : null,
				],
			},
		],
	});
	const allEventLogs = mostRecentFirst(
		parseEventLogs({
			// <https://github.com/wevm/viem/issues/4340>
			logs: rawLogs.map((log) => formatLog(log)),
			abi: consensusAbi,
			eventName: ["TransactionProposed", "TransactionAttested"],
			strict: true,
		}),
	);

	// With an explicit allow-list, `eth_getLogs` already filtered to just those oracles above; this
	// re-derives the same set from the (now pre-filtered) results as a cheap defensive check.
	// Without an allow-list, trust is derived after decoding: an oracle is trusted once it has a
	// `TransactionAttested` log in this same batch. Addresses are compared via their checksummed
	// form: `oracles` comes from user settings and may not be checksummed, while addresses decoded
	// from logs by viem always are.
	const trustedOracles = new Set(
		(oracles.length > 0
			? oracles
			: allEventLogs.filter((log) => log.eventName === "TransactionAttested").map((log) => log.args.oracle)
		).map((oracle) => getAddress(oracle)),
	);
	const eventLogs = allEventLogs.filter((log) => trustedOracles.has(getAddress(log.args.oracle)));

	const attestationKey = (log: { args: { safeTxHash: Hex; epoch: bigint; oracle: Address } }) =>
		`${log.args.safeTxHash}:${log.args.epoch}:${getAddress(log.args.oracle)}`;
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

			const oracle = log.args.oracle;
			const attestedAt = attestations.get(attestationKey(log)) ?? null;
			const proposedAt = { block: log.blockNumber, tx: log.transactionHash };
			const status: ProposalStatus =
				attestedAt !== null
					? "ATTESTED"
					: toBlock - proposedAt.block > BigInt(signingTimeout)
						? "TIMED_OUT"
						: "PROPOSED";
			return {
				chainId: transaction.data.chainId,
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
