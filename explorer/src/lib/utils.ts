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

export const getFromBlock = async (
	provider: PublicClient,
	maxBlockRange: bigint,
	referenceBlock?: bigint,
): Promise<bigint> => {
	const blockNumber = referenceBlock ?? (await provider.getBlockNumber());
	return blockNumber > maxBlockRange ? blockNumber - maxBlockRange : 0n;
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
