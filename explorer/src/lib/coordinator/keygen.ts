import { type Address, type Hex, type PublicClient, getAbiItem } from "viem";
import { COORDINATOR_KEY_GEN_EVENTS } from "@/lib/coordinator/abi";
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
	startBlock,
	endBlock,
}: {
	provider: PublicClient;
	consensus: Address;
	gid: Hex;
	startBlock: bigint;
	endBlock: bigint;
}): Promise<KeyGenStatus | null> => {
	const coordinator = await loadCoordinator(provider, consensus);

	const keyGenLogs = await provider.getLogs({
		address: coordinator,
		event: getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGen" }),
		args: { gid },
		fromBlock: startBlock,
		toBlock: endBlock,
		strict: true,
	});

	if (keyGenLogs.length === 0) return null;

	const keyGen = keyGenLogs[0];
	const count = keyGen.args.count;
	const threshold = keyGen.args.threshold;

	const [committedLogs, sharedLogs, confirmedLogs, complainedLogs] = await Promise.all([
		provider.getLogs({
			address: coordinator,
			event: getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenCommitted" }),
			args: { gid },
			fromBlock: startBlock,
			toBlock: endBlock,
			strict: true,
		}),
		provider.getLogs({
			address: coordinator,
			event: getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenSecretShared" }),
			args: { gid },
			fromBlock: startBlock,
			toBlock: endBlock,
			strict: true,
		}),
		provider.getLogs({
			address: coordinator,
			event: getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenConfirmed" }),
			args: { gid },
			fromBlock: startBlock,
			toBlock: endBlock,
			strict: true,
		}),
		provider.getLogs({
			address: coordinator,
			event: getAbiItem({ abi: COORDINATOR_KEY_GEN_EVENTS, name: "KeyGenComplained" }),
			args: { gid },
			fromBlock: startBlock,
			toBlock: endBlock,
			strict: true,
		}),
	]);

	const toParticipation = (log: { args: { identifier: bigint }; blockNumber: bigint }): KeyGenParticipation => ({
		identifier: log.args.identifier,
		block: log.blockNumber,
	});

	const committed = committedLogs.filter((l) => l.args.committed).map(toParticipation);
	const shared = sharedLogs.filter((l) => l.args.shared).map(toParticipation);
	const confirmed = confirmedLogs.filter((l) => l.args.confirmed).map(toParticipation);
	const compromised = complainedLogs.some((l) => l.args.compromised);
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
