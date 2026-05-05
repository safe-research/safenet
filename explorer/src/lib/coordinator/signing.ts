import { type Address, encodePacked, formatLog, type Hex, numberToHex, type PublicClient, parseEventLogs } from "viem";
import { consensusAbi } from "@/lib/consensus";
import {
	COORDINATOR_ABI,
	COORDINATOR_SIGNING_INITIATED_EVENT,
	COORDINATOR_SIGNING_PROGRESS_EVENTS,
	COORDINATOR_SIGNING_PROGRESS_SELECTORS,
} from "@/lib/coordinator/abi";
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
const groupKeyCache = new Map<string, Point>();

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

export type Point = {
	x: bigint;
	y: bigint;
};

export type Signature = {
	r: Point;
	z: bigint;
};

type AttestationInfo = {
	sid: Hex;
	groupId: Hex;
	sequence: bigint;
	lastUpdate: bigint;
	committed: AttestationParticipation[];
	signed: AttestationParticipation[];
};

export type AttestationStatus = AttestationInfo &
	(
		| {
				status: "completed";
				signature: Hex;
		  }
		| {
				status: "pending" | "error";
		  }
	);

type StatusAggregation = {
	lastUpdate: bigint;
	committed: AttestationParticipation[];
	signedBySelection: Record<string, AttestationParticipation[]>;
	selectionRoot?: Hex;
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
					status.signature = log.args.signature;
					break;
				}
			}
			agg[log.args.sid] = status;
			return agg;
		},
		{} as Record<string, StatusAggregation>,
	);
	// There is always at least one entry, due to the check on signingEvents before
	const [attestationStatus, signature] = signingEvents
		.map((signingEvent): [AttestationInfo, Signature | undefined] => {
			const sid = signingEvent.args.sid;
			const status = aggregate[sid];
			const groupId = signingEvent.args.gid;
			const sequence = signingEvent.args.sequence;
			const committed = status?.committed ?? [];
			const signed = getSigned(status);
			return [
				{
					lastUpdate: status.lastUpdate ?? signingEvent.blockNumber,
					sid,
					groupId,
					sequence,
					committed,
					signed,
				},
				status.signature,
			];
		})
		.sort((left, right) => {
			// Completed signing status always has priority
			if (left[1] !== undefined && right[1] === undefined) return -1;
			if (left[1] === undefined && right[1] !== undefined) return 1;
			// If both or none are completed return the recentrly updated one
			return left[0].lastUpdate < right[0].lastUpdate ? 1 : -1;
		})[0];

	if (!signature) return { ...attestationStatus, status: "pending" };

	const groupKey = await loadGroupPublicKey(provider, coordinator, attestationStatus.groupId);
	if (groupKey === undefined) return { ...attestationStatus, status: "error" };

	const isValid = verifyAttestationSignature(groupKey, signature, message);
	if (!isValid) {
		// Log and then remove invalid signature
		console.error(`Detected invalid signature ${signature} for ${attestationStatus.sid}`);
		return { ...attestationStatus, status: "completed", signature: formatSignatureHex(signature) };
	}

	return { ...attestationStatus, status: "error" };
};

const verifyAttestationSignature = (groupKey: Point, signature: Signature, message: Hex): boolean => {
	try {
		return verifySignature(toPoint(signature.r), signature.z, toPoint(groupKey), message);
	} catch (error) {
		// Log invalid signature
		console.error("Could not verify signature", error);
		return false;
	}
};

export const formatSignatureHex = (signature: Signature): Hex => {
	return encodePacked(["uint256", "uint256", "uint256"], [signature.r.x, signature.r.y, signature.z]);
};

export const loadGroupPublicKey = async (
	provider: PublicClient,
	coordinator: Address,
	groupId: Hex,
): Promise<Point | undefined> => {
	const cacheKey = `${coordinator}:${groupId}`;
	const cached = groupKeyCache.get(cacheKey);
	if (cached) {
		return cached;
	}

	try {
		const result = (await provider.readContract({
			address: coordinator,
			abi: COORDINATOR_ABI,
			functionName: "groupKey",
			args: [groupId],
		})) as Point;
		groupKeyCache.set(cacheKey, result);
		return result;
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
