import { type Address, formatLog, type Hex, numberToHex, type PublicClient, parseEventLogs } from "viem";
import { COORDINATOR_KEY_GEN_EVENTS, COORDINATOR_KEY_GEN_SELECTORS } from "@/lib/coordinator/abi";
import { loadCoordinator } from "@/lib/coordinator/signing";

export type KeyGenParticipation = {
	identifier: bigint;
	block: bigint;
};

export type KeyGenStatus = {
	gid: Hex;
	count: number;
	threshold: number;
	startBlock: bigint;
	committed: KeyGenParticipation[];
	shared: KeyGenParticipation[];
	confirmed: KeyGenParticipation[];
	finalized: boolean;
	compromised: boolean;
};

export const loadKeyGenDetails = async ({
	provider,
	consensus,
	gid,
	endBlock,
	blocksPerEpoch,
	prevStagedAt,
	maxBlockRange,
}: {
	provider: PublicClient;
	consensus: Address;
	gid: Hex;
	endBlock: bigint;
	blocksPerEpoch?: number;
	prevStagedAt?: bigint;
	maxBlockRange: bigint;
}): Promise<KeyGenStatus | null> => {
	const startBlock = computeStartBlock(endBlock, blocksPerEpoch, prevStagedAt, maxBlockRange);
	const coordinator = await loadCoordinator(provider, consensus);

	const logs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: coordinator,
				topics: [COORDINATOR_KEY_GEN_SELECTORS, [gid]],
				fromBlock: numberToHex(startBlock),
				toBlock: numberToHex(endBlock),
			},
		],
	});

	const eventLogs = parseEventLogs({
		logs: logs.map((log) => formatLog(log)),
		abi: COORDINATOR_KEY_GEN_EVENTS,
		strict: true,
	});

	const keyGenLog = eventLogs.find((log) => log.eventName === "KeyGen");
	if (!keyGenLog) return null;

	const count = keyGenLog.args.count;
	const threshold = keyGenLog.args.threshold;

	const toParticipation = (log: { args: { identifier: bigint }; blockNumber: bigint }): KeyGenParticipation => ({
		identifier: log.args.identifier,
		block: log.blockNumber,
	});

	const committed: KeyGenParticipation[] = [];
	const shared: KeyGenParticipation[] = [];
	const confirmed: KeyGenParticipation[] = [];
	let compromised = false;

	for (const log of eventLogs) {
		switch (log.eventName) {
			case "KeyGenCommitted": {
				if (log.args.committed) committed.push(toParticipation(log));
				break;
			}
			case "KeyGenSecretShared": {
				if (log.args.shared) shared.push(toParticipation(log));
				break;
			}
			case "KeyGenConfirmed": {
				if (log.args.confirmed) confirmed.push(toParticipation(log));
				break;
			}
			case "KeyGenComplained": {
				if (log.args.compromised) compromised = true;
				break;
			}
		}
	}

	const finalized = !compromised && confirmed.length >= threshold;

	return {
		gid,
		count,
		threshold,
		startBlock,
		committed,
		shared,
		confirmed,
		finalized,
		compromised,
	};
};

function computeStartBlock(
	endBlock: bigint,
	blocksPerEpoch?: number,
	prevStagedAt?: bigint,
	maxBlockRange?: bigint,
): bigint {
	if (blocksPerEpoch) {
		const bpe = BigInt(blocksPerEpoch);
		return endBlock - (endBlock % bpe);
	}
	if (prevStagedAt !== undefined) {
		return prevStagedAt;
	}
	const fallback = maxBlockRange ?? 10000n;
	return endBlock > fallback ? endBlock - fallback : 0n;
}
