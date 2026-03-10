import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import type { Log, PublicClient } from "viem";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export function jsonReplacer(_key: string, value: unknown): unknown {
	if (typeof value === "bigint") {
		return value.toString();
	}
	return value;
}

export type BlockRange = { fromBlock: bigint; toBlock: bigint };

export const getBlockRange = async (
	provider: PublicClient,
	maxBlockRange: bigint,
	referenceBlock?: bigint,
): Promise<BlockRange> => {
	const toBlock = referenceBlock ?? (await provider.getBlockNumber());
	const fromBlock = toBlock > maxBlockRange ? toBlock - maxBlockRange : 0n;
	return { fromBlock, toBlock };
};

export const mostRecentFirst = <T extends Pick<Log<bigint, number, false>, "blockNumber" | "logIndex">>(
	logs: T[],
): T[] =>
	logs.sort((left, right) => {
		if (left.blockNumber !== right.blockNumber) {
			return left.blockNumber < right.blockNumber ? 1 : -1;
		}
		return right.logIndex - left.logIndex;
	});
