import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import { createPublicClient, http, type Log, type PublicClient } from "viem";

/**
 * Returns `"#ffffff"` or `"#000000"` depending on which gives better contrast
 * against the given hex background colour (WCAG relative-luminance formula).
 */
export const contrastColor = (hex: string): "#ffffff" | "#000000" => {
	const r = Number.parseInt(hex.slice(1, 3), 16) / 255;
	const g = Number.parseInt(hex.slice(3, 5), 16) / 255;
	const b = Number.parseInt(hex.slice(5, 7), 16) / 255;
	const linearize = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
	const L = 0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b);
	return L < 0.179 ? "#ffffff" : "#000000";
};

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export function jsonReplacer(_key: string, value: unknown): unknown {
	if (typeof value === "bigint") {
		return value.toString();
	}
	return value;
}

export const createRpcClient = (rpc: string): PublicClient => createPublicClient({ transport: http(rpc) });

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
