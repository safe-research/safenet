import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import type { Log, PublicClient } from "viem";

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

/** Nominal seed for the block-at-timestamp estimate; refinement corrects for the real cadence. */
const BLOCK_TIME_SEED_SECONDS = 5;
/** Stop refining once a probe lands within this many seconds of the target. */
const BLOCK_ESTIMATE_TOLERANCE_SECONDS = 600;
/** Max refinement round-trips after the first probe (one cheap `eth_getBlockByNumber` each). */
const BLOCK_ESTIMATE_MAX_REFINEMENTS = 8;
/** How far behind the estimated block a targeted window starts, absorbing estimate error. */
const TARGETED_WINDOW_BACK_BLOCKS = 1000n;

/**
 * Estimate the block nearest a unix timestamp via secant refinement: the first step
 * uses the block time observed between the probe and the head, later steps the local
 * cadence between the last two probes, so the estimate converges even when the real
 * cadence drifts from the nominal seed or shifted over the chain's history.
 * `driftSeconds` is measured at the returned block and is `Infinity` when no probe
 * succeeded.
 */
export const estimateBlockAt = async (
	provider: PublicClient,
	targetSeconds: number,
	head: { number: bigint; timestamp: number },
): Promise<{ block: bigint; driftSeconds: number }> => {
	const headNumber = Number(head.number);
	const clamp = (block: number) => Math.min(headNumber, Math.max(0, Math.round(block)));
	let guess = clamp(headNumber - (head.timestamp - targetSeconds) / BLOCK_TIME_SEED_SECONDS);
	let best = { block: guess, driftSeconds: Number.POSITIVE_INFINITY };
	let prev: { block: number; timestamp: number } | null = null;

	for (let probe = 0; probe <= BLOCK_ESTIMATE_MAX_REFINEMENTS; probe++) {
		// A failed probe (e.g. a pruned header on public RPC) degrades to the caller's
		// head-relative scan rather than failing the whole read.
		const block = await provider.getBlock({ blockNumber: BigInt(guess) }).catch(() => null);
		if (!block) break;
		const timestamp = Number(block.timestamp);
		const driftSeconds = timestamp - targetSeconds;
		if (Math.abs(driftSeconds) < Math.abs(best.driftSeconds)) best = { block: guess, driftSeconds };
		if (Math.abs(driftSeconds) <= BLOCK_ESTIMATE_TOLERANCE_SECONDS) break;
		// Consecutive probes bracket the target's cadence regime, so their secant slope
		// beats the whole-chain average; the first probe only has the head to lean on.
		const span = prev === null ? headNumber - guess : guess - prev.block;
		const rise = prev === null ? head.timestamp - timestamp : timestamp - prev.timestamp;
		// Floored so a run of equal timestamps cannot divide by zero below.
		const observedBlockTime = span !== 0 ? Math.max(rise / span, 0.1) : BLOCK_TIME_SEED_SECONDS;
		prev = { block: guess, timestamp };
		const next = clamp(guess - driftSeconds / observedBlockTime);
		if (next === guess) break;
		guess = next;
	}
	return { block: BigInt(best.block), driftSeconds: best.driftSeconds };
};

/**
 * A second log window aimed at `timestampSeconds`, for events that fell out of the
 * head-relative window. Returns `null` when the head window already reaches genesis,
 * the estimate did not converge, or the target is recent enough that the aimed window
 * would overlap the head-relative one — callers then keep their current behaviour.
 */
export const getTargetedBlockRange = async (
	provider: PublicClient,
	maxBlockRange: bigint,
	timestampSeconds: number,
	headRange: BlockRange,
): Promise<BlockRange | null> => {
	if (headRange.fromBlock === 0n) return null;
	const head = await provider.getBlock({ blockNumber: headRange.toBlock }).catch(() => null);
	if (head === null) return null;
	const { block, driftSeconds } = await estimateBlockAt(provider, timestampSeconds, {
		number: headRange.toBlock,
		timestamp: Number(head.timestamp),
	});
	if (Math.abs(driftSeconds) > BLOCK_ESTIMATE_TOLERANCE_SECONDS) return null;
	const fromBlock = block > TARGETED_WINDOW_BACK_BLOCKS ? block - TARGETED_WINDOW_BACK_BLOCKS : 0n;
	const toBlock =
		headRange.fromBlock - 1n < fromBlock + maxBlockRange ? headRange.fromBlock - 1n : fromBlock + maxBlockRange;
	return toBlock >= fromBlock ? { fromBlock, toBlock } : null;
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

let cachedChainId: { provider: PublicClient; chainId: Promise<number> } | undefined;

export const loadChainId = async (provider: PublicClient): Promise<number> => {
	if (provider !== cachedChainId?.provider) {
		const entry: { provider: PublicClient; chainId: Promise<number> } = {
			provider,
			chainId: provider.getChainId().catch((error) => {
				// Don't cache errors
				if (cachedChainId === entry) {
					cachedChainId = undefined;
				}
				throw error;
			}),
		};
		cachedChainId = entry;
	}
	return cachedChainId.chainId;
};
