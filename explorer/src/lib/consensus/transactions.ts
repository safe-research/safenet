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
import { oracleAbi, oracleResultEventSelector } from "@/lib/oracle/abi";
import { oracleRequestId } from "@/lib/oracle/hashing";
import { bigIntSchema, checkedAddressSchema, hexDataSchema } from "@/lib/schemas";
import { getBlockRange, jsonReplacer, loadChainId, mostRecentFirst, oldestFirst } from "@/lib/utils";
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
	oracleData: Hex;
	// The key the oracle tracks this proposal's request under, and the message the validators
	// attest — see `oracleRequestId`. Derived from the fields above plus the consensus chain.
	requestId: Hex;
	transaction: SafeTransaction;
	proposedAt: ExecutionLink;
	attestedAt: ExecutionLink | null;
};

// Lifecycle of a proposal, as far as the explorer can observe it on-chain:
//
//   PROPOSED ──no oracle verdict in time──────────────────────────────> TIMED_OUT (final)
//     │
//     ├──`OracleResult(approved: false)`──────────────────────────────> DENIED (final)
//     │
//     └──`OracleResult(approved: true)`─> APPROVED ──`TransactionAttested`──> ATTESTED (final)
//                                            └──no attestation in time──────> TIMED_OUT (final)
//
// `TIMED_OUT` is an error state: it means the explorer expected something to happen and it
// didn't, so it is only reported when no verdict/attestation explains the silence. A denied
// proposal is a normal, final outcome — no attestation is ever expected for it.
export type ProposalStatus = "PROPOSED" | "APPROVED" | "ATTESTED" | "DENIED" | "TIMED_OUT";

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

// Identifies one proposal across the consensus events and the oracle's verdict: the same Safe
// transaction can be re-proposed in a later epoch, and every oracle tracks its own request.
const proposalKey = ({ safeTxHash, epoch, oracle }: { safeTxHash: Hex; epoch: bigint; oracle: Address }) =>
	`${safeTxHash}:${epoch}:${getAddress(oracle)}`;

type OracleVerdict = { approved: boolean; resolvedAt: ExecutionLink };

// Loads oracle verdicts from a single `eth_getLogs`, keyed by the request ID the oracle tracks
// each proposal under. `requestIds` narrows the query to specific proposals; passing none returns
// every verdict the given oracles emitted in the range instead, which is what the unscoped
// overview wants — there the topic list would grow with every proposal in the block range, and
// callers look verdicts up by request ID either way, so a broader query only adds ignored logs.
const loadOracleVerdicts = async ({
	provider,
	oracles,
	requestIds,
	fromBlock,
	toBlock,
}: {
	provider: PublicClient;
	oracles: Address[];
	requestIds: Hex[];
	fromBlock: bigint;
	toBlock: bigint;
}): Promise<Map<Hex, OracleVerdict>> => {
	const rawLogs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: oracles,
				fromBlock: numberToHex(fromBlock),
				toBlock: numberToHex(toBlock),
				topics: [oracleResultEventSelector, requestIds.length > 0 ? requestIds : null],
			},
		],
	});
	const logs = parseEventLogs({
		logs: rawLogs.map((log) => formatLog(log)),
		abi: oracleAbi,
		eventName: "OracleResult",
		strict: true,
	});
	// Oldest verdict first, so that for a request that somehow resolved more than once the newest
	// one wins the `Map` insertion below.
	return new Map(
		oldestFirst(logs).map(
			(log) =>
				[
					log.args.requestId,
					{ approved: log.args.approved, resolvedAt: { block: log.blockNumber, tx: log.transactionHash } },
				] as const,
		),
	);
};

const deriveProposalStatus = ({
	proposedAt,
	attestedAt,
	verdict,
	isTimedOut,
}: {
	proposedAt: ExecutionLink;
	attestedAt: ExecutionLink | null;
	verdict: OracleVerdict | undefined;
	isTimedOut: (since: bigint) => boolean;
}): ProposalStatus => {
	if (attestedAt !== null) {
		return "ATTESTED";
	}
	// No verdict: still waiting on the oracle. An oracle that gave up doesn't emit `OracleResult`
	// (`SentinelOracle` emits `RequestTimedOut`, which isn't read here), so silence that outlasts
	// the timeout is all there is to go on.
	if (verdict === undefined) {
		return isTimedOut(proposedAt.block) ? "TIMED_OUT" : "PROPOSED";
	}
	// Final: `Consensus` never attests a denied proposal, so nothing is outstanding to time out.
	if (!verdict.approved) {
		return "DENIED";
	}
	return isTimedOut(verdict.resolvedAt.block) ? "TIMED_OUT" : "APPROVED";
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
	// The chain ID is only needed to derive each proposal's `requestId` below; it's resolved
	// alongside the block range (and cached per provider) rather than serially before it.
	const [{ fromBlock, toBlock }, chainId] = await Promise.all([
		getBlockRange(provider, maxBlockRange, referenceBlock),
		loadChainId(provider),
	]);
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

	const attestations = new Map(
		eventLogs
			.filter((log) => log.eventName === "TransactionAttested")
			.map((log) => [proposalKey(log.args), { block: log.blockNumber, tx: log.transactionHash }] as const),
	);
	const proposed = eventLogs.flatMap((log) => {
		if (log.eventName !== "TransactionProposed") {
			return [];
		}

		const transaction = safeTransactionSchema.safeParse(log.args.transaction);
		if (!transaction.success) {
			return [];
		}

		const { safeTxHash: proposedTxHash, epoch, oracle, oracleData } = log.args;
		return [
			{
				chainId: transaction.data.chainId,
				safeTxHash: proposedTxHash,
				epoch,
				oracle,
				oracleData,
				requestId: oracleRequestId({ chainId, consensus, epoch, oracle, oracleData, safeTxHash: proposedTxHash }),
				transaction: transaction.data,
				proposedAt: { block: log.blockNumber, tx: log.transactionHash },
				attestedAt: attestations.get(proposalKey(log.args)) ?? null,
			},
		];
	});

	// The requests whose verdict still matters: an attestation already tells the whole story, so a
	// fully attested batch skips the oracle query altogether.
	const oracleVerdictRequests = proposed.flatMap(({ attestedAt, requestId }) =>
		attestedAt === null ? [requestId] : [],
	);
	const verdicts =
		oracleVerdictRequests.length > 0
			? await loadOracleVerdicts({
					provider,
					oracles: [...trustedOracles],
					// Same scoping as the consensus query above: with a `safeTxHash` or a `safeId` these
					// are a handful of request IDs worth filtering on, without either the unscoped
					// overview takes every verdict in the range instead.
					requestIds: safeTxHash !== undefined || safeId !== undefined ? oracleVerdictRequests : [],
					fromBlock,
					toBlock,
				})
			: new Map<Hex, OracleVerdict>();

	// Each phase gets its own `signingTimeout` budget: waiting on the oracle is measured from the
	// proposal, waiting on the validators from the oracle's verdict.
	const isTimedOut = (since: bigint) => toBlock - since > BigInt(signingTimeout);
	const proposals = proposed.map((proposal) => ({
		...proposal,
		status: deriveProposalStatus({ ...proposal, verdict: verdicts.get(proposal.requestId), isTimedOut }),
	}));

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
