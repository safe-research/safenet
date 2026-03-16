import { type Address, getAbiItem, type Hex, type PublicClient } from "viem";
import { getBlockRange, mostRecentFirst } from "@/lib/utils";
import { consensusAbi } from "./abi";

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

export type EpochRolloverResult = {
	entries: EpochRolloverEntry[];
	reachedGenesis: boolean;
	fromBlock: bigint;
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
