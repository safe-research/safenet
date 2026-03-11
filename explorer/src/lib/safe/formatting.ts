import { formatUnits, type Hex, size } from "viem";
import type { ChainInfo } from "@/lib/chains";

export type Currency = {
	name: string;
	/** 2-6 characters long */
	symbol: string;
	decimals: number;
};

export const opString = (operation: 0 | 1) => (operation === 0 ? "CALL" : "DELEGATECALL");
export const valueString = (value: bigint, currency?: Currency) =>
	`${formatUnits(value, currency?.decimals ?? 18)} ${currency?.symbol ?? "ETH"}`;
export const dataString = (data: Hex) => `${size(data)} bytes of data`;

/** Returns `0x` + first 8 hex chars + `…` + last 8 hex chars. Returns the full hash if truncation would not shorten it. */
export const formatHashShort = (hash: Hex): string => {
	const hex = hash.slice(2);
	if (hex.length <= 16) return hash;
	return `0x${hex.slice(0, 8)}…${hex.slice(-8)}`;
};

/** Converts a block-count difference to a human-readable age string using the chain's block time. */
export const formatBlockAge = (blockDiff: bigint, chain: ChainInfo, now?: number): string => {
	const blockTimeMs = BigInt(chain.blockTime ?? 12_000);
	const seconds = (blockDiff * blockTimeMs) / 1000n;
	if (seconds < 60n) return `${seconds}s ago`;
	if (seconds < 3600n) return `${seconds / 60n}m ago`;
	const date = new Date((now ?? Date.now()) - Number(seconds) * 1000);
	return date.toISOString().slice(0, 10);
};
