import { type Address, formatLog, type Hex, numberToHex, type PublicClient, parseAbi, parseEventLogs } from "viem";
import {
	COORDINATOR_SIGNING_INITIATED_EVENT,
	COORDINATOR_SIGNING_PROGRESS_EVENTS,
	COORDINATOR_SIGNING_PROGRESS_SELECTORS,
} from "@/lib/coordinator/abi";
import { safeTxProposalHash } from "@/lib/packets";
import { getFromBlock } from "@/lib/utils";

let cachedAddresses:
	| {
			consensus: Address;
			coordinator: Promise<Address>;
	  }
	| undefined;

const COORDINATOR_UPPER_ABI = parseAbi(["function COORDINATOR() view returns (address)"]);
const GET_COORDINATOR_ABI = parseAbi(["function getCoordinator() view returns (address)"]);

const fetchCoordinator = async (provider: PublicClient, consensus: Address): Promise<Address> => {
	// Attempt to batch both calls in a single Multicall3 RPC round-trip.
	// If multicall itself throws (e.g. Multicall3 not deployed), fall back to individual calls.
	let multicallResults:
		| ReadonlyArray<{ status: "success"; result: Address } | { status: "failure"; error: Error }>
		| undefined;
	try {
		multicallResults = await provider.multicall({
			contracts: [
				{ address: consensus, abi: COORDINATOR_UPPER_ABI, functionName: "COORDINATOR" },
				{ address: consensus, abi: GET_COORDINATOR_ABI, functionName: "getCoordinator" },
			],
			allowFailure: true,
		});
	} catch {
		// Multicall3 not available on this chain — fall back to individual calls
	}

	if (multicallResults !== undefined) {
		const [upper, getter] = multicallResults;
		// Prefer getCoordinator(), fall back to COORDINATOR()
		if (getter.status === "success") return getter.result;
		if (upper.status === "success") return upper.result;
		throw new Error(`Could not read coordinator from consensus contract ${consensus}`);
	}

	const [getterResult, upperResult] = await Promise.allSettled([
		provider.readContract({ address: consensus, abi: GET_COORDINATOR_ABI, functionName: "getCoordinator" }),
		provider.readContract({ address: consensus, abi: COORDINATOR_UPPER_ABI, functionName: "COORDINATOR" }),
	]);
	if (getterResult.status === "fulfilled") return getterResult.value;
	if (upperResult.status === "fulfilled") return upperResult.value;
	throw new Error(`Could not read coordinator from consensus contract ${consensus}`);
};

const loadCoordinator = (provider: PublicClient, consensus: Address): Promise<Address> => {
	if (cachedAddresses?.consensus === consensus) {
		return cachedAddresses.coordinator;
	}

	const coordinator = fetchCoordinator(provider, consensus);
	cachedAddresses = { consensus, coordinator };
	return coordinator;
};

export type AttestationParticipation = {
	identifier: bigint;
	block: bigint;
};

export type AttestationStatus = {
	sid: Hex;
	groupId: Hex;
	sequence: bigint;
	lastUpdate: bigint;
	committed: AttestationParticipation[];
	signed: AttestationParticipation[];
	completed: boolean;
};

export const loadLatestAttestationStatus = async ({
	provider,
	consensus,
	safeTxHash,
	epoch,
	proposedAt,
	attestedAt,
	maxBlockRange,
}: {
	provider: PublicClient;
	consensus: Address;
	safeTxHash: Hex;
	epoch: bigint;
	proposedAt?: bigint;
	attestedAt?: bigint | null;
	maxBlockRange: bigint;
}): Promise<AttestationStatus | null> => {
	// We use an `eth_getLogs` here directly, in order to filter on the `transactionHash` of both `TransactionProposed`
	// and `TransactionAttested` events.
	const fromBlock = proposedAt ?? (await getFromBlock(provider, maxBlockRange));
	const toBlock = attestedAt ?? "latest";
	const chainId = await provider.getChainId();
	const coordinator = await loadCoordinator(provider, consensus);
	const message = safeTxProposalHash({
		domain: {
			chainId,
			verifyingContract: consensus,
		},
		proposal: {
			epoch,
			safeTxHash,
		},
	});
	// Get signing events related to this message
	const signingEvents = await provider.getLogs({
		address: coordinator,
		event: COORDINATOR_SIGNING_INITIATED_EVENT,
		args: {
			message,
		},
		fromBlock,
		toBlock,
		strict: true,
	});
	if (signingEvents.length === 0) return null;
	const signingIds = signingEvents.map((e) => e.args.sid);
	const logs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: coordinator,
				topics: [COORDINATOR_SIGNING_PROGRESS_SELECTORS, signingIds],
				fromBlock: numberToHex(fromBlock),
				toBlock: typeof toBlock === "bigint" ? numberToHex(toBlock) : toBlock,
			},
		],
	});
	const eventLogs = parseEventLogs({
		// <https://github.com/wevm/viem/issues/4340>
		logs: logs.map((log) => formatLog(log)),
		abi: COORDINATOR_SIGNING_PROGRESS_EVENTS,
		strict: true,
	});

	const aggregate = eventLogs.reduce(
		(agg, log) => {
			const status = agg[log.args.sid] ?? { committed: [], signedBySelection: {}, lastUpdate: 0n };
			if (status.lastUpdate < log.blockNumber) {
				status.lastUpdate = log.blockNumber;
			}
			switch (log.eventName) {
				case "SignRevealedNonces": {
					status.committed.push({
						identifier: log.args.identifier,
						block: log.blockNumber,
					});
					break;
				}
				case "SignShared": {
					const shares = status.signedBySelection[log.args.selectionRoot] ?? [];
					shares.push({
						identifier: log.args.identifier,
						block: log.blockNumber,
					});
					status.signedBySelection[log.args.selectionRoot] = shares;
					break;
				}
				case "SignCompleted": {
					status.selectionRoot = log.args.selectionRoot;
					break;
				}
			}
			agg[log.args.sid] = status;
			return agg;
		},
		{} as Record<string, StatusAggregation>,
	);
	// There is always at least one entry, due to the check on signingEvents before
	return signingEvents
		.map((signingEvent) => {
			const sid = signingEvent.args.sid;
			const status = aggregate[sid];
			const groupId = signingEvent.args.gid;
			const sequence = signingEvent.args.sequence;
			const committed = status?.committed ?? [];
			const signed = getSigned(status);
			return {
				lastUpdate: status.lastUpdate ?? signingEvent.blockNumber,
				sid,
				groupId,
				sequence,
				committed,
				signed,
				completed: status.selectionRoot !== undefined,
			};
		})
		.sort((left, right) => {
			// Completed signing status always has priority
			if (left.completed && !right.completed) return -1;
			if (!left.completed && right.completed) return 1;
			// If both or none are completed return the recentrly updated one
			return left.lastUpdate < right.lastUpdate ? 1 : -1;
		})[0];
};

type StatusAggregation = {
	lastUpdate: bigint;
	selectionRoot?: Hex;
	committed: AttestationParticipation[];
	signedBySelection: Record<string, AttestationParticipation[]>;
};

const getSigned = (status: StatusAggregation | undefined): AttestationParticipation[] => {
	if (status === undefined) {
		return [];
	}
	if (status.selectionRoot !== undefined) {
		return status.signedBySelection[status.selectionRoot] ?? [];
	}
	// If not completed look for the selection with the most signatures
	return Object.values(status.signedBySelection).reduce((left, right) => {
		return left.length >= right.length ? left : right;
	}, []);
};
