import { type Address, formatLog, type Hex, numberToHex, type PublicClient, parseEventLogs } from "viem";
import { consensusAbi } from "@/lib/consensus";
import {
	COORDINATOR_SIGNING_INITIATED_EVENT,
	COORDINATOR_SIGNING_PROGRESS_EVENTS,
	COORDINATOR_SIGNING_PROGRESS_SELECTORS,
	GROUP_KEY_ABI,
} from "@/lib/coordinator/abi";
import type { FrostPoint } from "@/lib/frost/math";
import { toPoint } from "@/lib/frost/math";
import { verifySignature } from "@/lib/frost/verify";
import { safeTxProposalHash } from "@/lib/packets";
import { getBlockRange } from "@/lib/utils";

let cachedAddresses:
	| {
			consensus: Address;
			coordinator: Promise<Address>;
	  }
	| undefined;

// Cache for group keys per groupId
const groupKeyCache = new Map<string, FrostPoint>();

const fetchCoordinator = (provider: PublicClient, consensus: Address): Promise<Address> => {
	return provider.readContract({ address: consensus, abi: consensusAbi, functionName: "getCoordinator" });
};

export const loadCoordinator = (provider: PublicClient, consensus: Address): Promise<Address> => {
	if (cachedAddresses?.consensus === consensus) {
		return cachedAddresses.coordinator;
	}

	const coordinator = fetchCoordinator(provider, consensus).catch((err) => {
		// Don't permanently cache failures — clear so the next call retries
		if (cachedAddresses?.consensus === consensus) cachedAddresses = undefined;
		throw err;
	});
	cachedAddresses = { consensus, coordinator };
	return coordinator;
};

export type AttestationParticipation = {
	address: Address;
	block: bigint;
};

export type Signature = {
	r: FrostPoint;
	z: bigint;
};

export type AttestationStatus = {
	sid: Hex;
	groupId: Hex;
	sequence: bigint;
	lastUpdate: bigint;
	committed: AttestationParticipation[];
	signed: AttestationParticipation[];
	completed: boolean;
	signature?: Signature;
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
	const { fromBlock: defaultFromBlock, toBlock } = await getBlockRange(
		provider,
		maxBlockRange,
		attestedAt ?? undefined,
	);
	const fromBlock = proposedAt ?? defaultFromBlock;
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
	// We use an `eth_getLogs` here directly, in order to filter on the `sid` of all the signing
	// process events.
	const logs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: coordinator,
				topics: [COORDINATOR_SIGNING_PROGRESS_SELECTORS, signingIds],
				fromBlock: numberToHex(fromBlock),
				toBlock: numberToHex(toBlock),
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
						address: log.args.participant,
						block: log.blockNumber,
					});
					break;
				}
				case "SignShared": {
					const shares = status.signedBySelection[log.args.selectionRoot] ?? [];
					shares.push({
						address: log.args.participant,
						block: log.blockNumber,
					});
					status.signedBySelection[log.args.selectionRoot] = shares;
					break;
				}
				case "SignCompleted": {
					status.selectionRoot = log.args.selectionRoot;
					status.signature = {
						r: toPoint(log.args.signature.r),
						z: log.args.signature.z,
					};
					break;
				}
			}
			agg[log.args.sid] = status;
			return agg;
		},
		{} as Record<string, StatusAggregation>,
	);
	// There is always at least one entry, due to the check on signingEvents before
	const attestationStatus = signingEvents
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
				signature: status.signature,
			};
		})
		.sort((left, right) => {
			// Completed signing status always has priority
			if (left.completed && !right.completed) return -1;
			if (!left.completed && right.completed) return 1;
			// If both or none are completed return the recentrly updated one
			return left.lastUpdate < right.lastUpdate ? 1 : -1;
		})[0];

	if (!attestationStatus.completed || !attestationStatus.signature) return attestationStatus;

	const groupKey = await loadGroupPublicKey(provider, coordinator, attestationStatus.groupId);

	if (groupKey === undefined) return attestationStatus;

	const signature = attestationStatus.signature;
	const isValid = verifySignature(signature.r, signature.z, groupKey, message);
	if (!isValid) {
		// Log and then remove invalid signature
		console.error(`Detected invalid signature ${signature} for ${attestationStatus.sid}`);
		attestationStatus.signature = undefined;
	}

	return attestationStatus;
};

type StatusAggregation = {
	lastUpdate: bigint;
	selectionRoot?: Hex;
	committed: AttestationParticipation[];
	signedBySelection: Record<string, AttestationParticipation[]>;
	signature?: Signature;
};

export const formatSignatureHex = (signature: Signature): Hex => {
	return `0x${[signature.r.x, signature.r.y, signature.z].map((v) => v.toString(16).padStart(64, "0")).join("")}`;
};

export const loadGroupPublicKey = async (
	provider: PublicClient,
	coordinator: Address,
	groupId: Hex,
): Promise<FrostPoint | undefined> => {
	const cacheKey = `${coordinator}:${groupId}`;
	const cached = groupKeyCache.get(cacheKey);
	if (cached) {
		return cached;
	}

	try {
		const result = (await provider.readContract({
			address: coordinator,
			abi: GROUP_KEY_ABI,
			functionName: "groupKey",
			args: [groupId],
		})) as { x: bigint; y: bigint };
		const publicKey = toPoint(result);
		groupKeyCache.set(cacheKey, publicKey);
		return publicKey;
	} catch (_error) {
		// Group might not exist or key generation might not be complete
		return undefined;
	}
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
