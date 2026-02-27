/**
 * Benchmark: measure the performance impact of indexes on nonces_links and nonces tables.
 *
 * Scenarios are calibrated to real-world values derived from the codebase and
 * the validator handbook:
 *
 *   - Epoch duration: ~1 day (17,280 blocks at 5s/block)
 *   - Nonce tree (chunk) size: 1,024 nonces (SEQUENCE_CHUNK_SIZE)
 *   - New chunk generated when available nonces < 100 (NONCE_THRESHOLD)
 *   - Groups are NEVER unregistered in production (only in tests)
 *   - Burned nonces stay as NULL'd rows (hiding/binding set to NULL, row persists)
 *   - Old nonce trees are NEVER deleted — they accumulate across epochs
 *   - Participants per group: ~3-10 in production
 *
 * This means:
 *   - Each epoch creates exactly 1 group (never cleaned up)
 *   - Each group starts with 1 chunk of 1,024 nonces, getting new chunks as needed
 *   - After N days, there are ~N groups + N–N*2 chunks of dead nonce data
 *
 * Run with:
 *   npx vitest run src/consensus/storage/sqlite-bench.test.ts
 */

import Sqlite3, { type Database } from "better-sqlite3";
import type { Address, Hex } from "viem";
import { describe, expect, it } from "vitest";
import { g } from "../../frost/math.js";
import type { NonceTree } from "../signing/nonces.js";
import { SqliteClientStorage } from "./sqlite.js";

// ---------------------------------------------------------------------------
// Real-world constants (from codebase)
// ---------------------------------------------------------------------------

const NONCES_PER_TREE = 1024; // SEQUENCE_CHUNK_SIZE in nonces.ts

const account: Address = "0x0000000000000000000000000000000000000001";
const participants = [
	{ id: 1n, address: "0x0000000000000000000000000000000000000001" as Address },
	{ id: 2n, address: "0x0000000000000000000000000000000000000002" as Address },
	{ id: 3n, address: "0x0000000000000000000000000000000000000003" as Address },
] as const;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const hex32 = (prefix: string, n: number): Hex => {
	// 16 hex chars for counter to support large scenarios (up to ~2^52 unique IDs)
	const hex = n.toString(16).padStart(16, "0");
	// 32 bytes = 64 hex chars: 48 from prefix + 16 from counter
	return `0x${prefix.repeat(24)}${hex}` as Hex;
};

// Pre-compute one set of EC points to reuse across all trees.
// The benchmark measures SQL / index performance, not EC math. Reusing points
// avoids spending minutes on `g(seed)` for 1M+ nonces while producing the
// same number of database rows.
const precomputedPoints = Array.from({ length: NONCES_PER_TREE }, (_, i) => {
	const seed = BigInt(i + 1);
	return {
		hidingNonceCommitment: g(seed),
		bindingNonceCommitment: g(seed + 100_000n),
	};
});

const makeTree = (groupIdx: number, treeIdx: number): NonceTree => {
	const base = groupIdx * 100_000 + treeIdx * 10_000;
	const root = hex32("aa", base);
	const leaves: Hex[] = [];
	const commitments = [];
	for (let i = 0; i < NONCES_PER_TREE; i++) {
		const seed = BigInt(base + i + 1);
		leaves.push(hex32("bb", base + i));
		commitments.push({
			hidingNonce: seed,
			hidingNonceCommitment: precomputedPoints[i].hidingNonceCommitment,
			bindingNonce: seed + 100_000n,
			bindingNonceCommitment: precomputedPoints[i].bindingNonceCommitment,
		});
	}
	return { root, leaves, commitments };
};

// ---------------------------------------------------------------------------
// Scenario definitions
// ---------------------------------------------------------------------------

/**
 * Each scenario models a realistic point in a validator's lifetime.
 *
 * Derivation:
 *   - 1 epoch ≈ 1 day → 1 group per day, never cleaned up
 *   - Active group: 1 chunk to start, +1 chunk every ~924 signatures
 *   - Old groups: chunks stay (dead weight — burned nonces are NULL'd rows)
 */
const scenarios = [
	{
		name: "Day 1 (fresh validator)",
		groups: 1,
		chunksPerGroup: 1,
		// 1 nonces_links, 1,024 nonces
	},
	{
		name: "Week 1 (~7 epochs, low volume)",
		groups: 7,
		chunksPerGroup: 1,
		// 7 nonces_links, 7,168 nonces
	},
	{
		name: "Month 1 (~30 epochs, moderate volume)",
		groups: 30,
		chunksPerGroup: 2,
		// 60 nonces_links, 61,440 nonces
	},
	{
		name: "Month 6 (end of beta, moderate volume)",
		groups: 180,
		chunksPerGroup: 2,
		// 360 nonces_links, 368,640 nonces
	},
	{
		name: "Year 1 (no cleanup, high volume)",
		groups: 365,
		chunksPerGroup: 3,
		// 1,095 nonces_links, 1,121,280 nonces ~ just over 1M rows
	},
] as const;

type ScenarioConfig = (typeof scenarios)[number];

// ---------------------------------------------------------------------------
// Database factory
// ---------------------------------------------------------------------------

type SeededStorage = {
	storage: SqliteClientStorage;
	db: Database;
	/** Lookups for the ACTIVE group only (last group = current epoch) */
	activeLookups: { groupId: Hex; chunk: bigint }[];
	totalNoncesLinks: number;
	totalNonces: number;
};

const createSeededStorage = (
	scenario: ScenarioConfig,
	withIndexes: boolean,
	options?: { burnOldGroups?: boolean },
): SeededStorage => {
	const db = new Sqlite3(":memory:");
	db.pragma("journal_mode = WAL");
	db.pragma("foreign_keys = ON");

	const storage = new SqliteClientStorage(account, db);

	if (!withIndexes) {
		db.exec("DROP INDEX IF EXISTS idx_nonces_links_lookup");
		db.exec("DROP INDEX IF EXISTS idx_nonces_root");
	}

	const activeLookups: { groupId: Hex; chunk: bigint }[] = [];
	const isActiveGroup = (gi: number) => gi === scenario.groups - 1;

	for (let gi = 0; gi < scenario.groups; gi++) {
		const groupId = hex32("ff", gi);
		storage.registerGroup(groupId, participants, 2);

		for (let ti = 0; ti < scenario.chunksPerGroup; ti++) {
			const tree = makeTree(gi, ti);
			storage.registerNonceTree(groupId, tree);
			const chunk = BigInt(ti);
			storage.linkNonceTree(groupId, chunk, tree.root);

			if (isActiveGroup(gi)) {
				activeLookups.push({ groupId, chunk });
			}
		}

		// Simulate real-world: old groups have fully burned nonces
		// (hiding/binding = NULL but rows remain — never deleted)
		// Use bulk SQL for speed — seeding is not what we're benchmarking.
		if (!isActiveGroup(gi) && options?.burnOldGroups !== false) {
			db.exec(`
				UPDATE nonces SET hiding = NULL, binding = NULL
				WHERE root IN (
					SELECT root FROM nonces_links
					WHERE group_id = '${groupId}' AND address = '${account}'
				)
			`);
		}
	}

	const totalNoncesLinks = scenario.groups * scenario.chunksPerGroup;
	const totalNonces = totalNoncesLinks * NONCES_PER_TREE;

	return { storage, db, activeLookups, totalNoncesLinks, totalNonces };
};

// ---------------------------------------------------------------------------
// Timing utility (raw SQL — isolates index impact from Zod parsing overhead)
// ---------------------------------------------------------------------------

const RAW_QUERY = `
	SELECT root, leaf, hiding, hiding_commitment, binding, binding_commitment
	FROM nonces
	WHERE root = (
		SELECT l.root FROM nonces_links AS l
		WHERE l.group_id = ? AND l.address = ? AND l.chunk = ?
	)
	ORDER BY offset ASC
`;

const timeRawQuery = (
	db: Database,
	lookups: { groupId: Hex; chunk: bigint }[],
	iterations: number,
): { totalMs: number; avgUs: number; opsPerSec: number } => {
	const stmt = db.prepare(RAW_QUERY);
	// Warmup
	for (let i = 0; i < Math.min(100, lookups.length); i++) {
		stmt.all(lookups[i].groupId, account, lookups[i].chunk);
	}
	const start = performance.now();
	for (let i = 0; i < iterations; i++) {
		const { groupId, chunk } = lookups[i % lookups.length];
		stmt.all(groupId, account, chunk);
	}
	const totalMs = performance.now() - start;
	return {
		totalMs,
		avgUs: (totalMs / iterations) * 1000,
		opsPerSec: Math.round((iterations / totalMs) * 1000),
	};
};

const BURN_QUERY = `
	UPDATE nonces
	SET hiding = NULL, binding = NULL
	WHERE root = (
		SELECT l.root FROM nonces_links AS l
		WHERE l.group_id = ? AND l.address = ? AND l.chunk = ?
	)
	AND offset = ? AND hiding IS NOT NULL AND binding IS NOT NULL
`;

const timeBurns = (
	db: Database,
	lookups: { groupId: Hex; chunk: bigint }[],
	noncesPerTree: number,
): { totalMs: number; avgUs: number; opsPerSec: number; count: number } => {
	const stmt = db.prepare(BURN_QUERY);
	const count = lookups.length * noncesPerTree;
	const start = performance.now();
	for (let offset = 0; offset < noncesPerTree; offset++) {
		for (const { groupId, chunk } of lookups) {
			stmt.run(groupId, account, chunk, offset);
		}
	}
	const totalMs = performance.now() - start;
	return {
		totalMs,
		avgUs: (totalMs / count) * 1000,
		opsPerSec: Math.round((count / totalMs) * 1000),
		count,
	};
};

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

const fmt = (n: number, decimals = 1) => n.toFixed(decimals);
const fmtInt = (n: number) => n.toLocaleString();

const printTable = (
	title: string,
	subtitle: string,
	withoutResult: { totalMs: number; avgUs: number; opsPerSec: number },
	withResult: { totalMs: number; avgUs: number; opsPerSec: number },
) => {
	const speedup = withoutResult.avgUs / withResult.avgUs;
	const w = 69;
	console.log(`\n┌${"─".repeat(w)}┐`);
	console.log(`│  ${title.padEnd(w - 2)}│`);
	console.log(`│  ${subtitle.padEnd(w - 2)}│`);
	console.log(`├${"─".repeat(22)}┬${"─".repeat(10)}┬${"─".repeat(11)}┬${"─".repeat(15)}┬${"─".repeat(7)}┤`);
	console.log(
		`│ ${"Variant".padEnd(20)} │ ${"Total ms".padStart(8)} │ ${"Avg (μs)".padStart(9)} │ ${"Ops/sec".padStart(13)} │ ${"Δ".padStart(5)} │`,
	);
	console.log(`├${"─".repeat(22)}┼${"─".repeat(10)}┼${"─".repeat(11)}┼${"─".repeat(15)}┼${"─".repeat(7)}┤`);
	console.log(
		`│ ${"WITHOUT indexes".padEnd(20)} │ ${fmt(withoutResult.totalMs).padStart(8)} │ ${fmt(withoutResult.avgUs, 2).padStart(9)} │ ${fmtInt(withoutResult.opsPerSec).padStart(13)} │ ${"".padStart(5)} │`,
	);
	console.log(
		`│ ${"WITH indexes".padEnd(20)} │ ${fmt(withResult.totalMs).padStart(8)} │ ${fmt(withResult.avgUs, 2).padStart(9)} │ ${fmtInt(withResult.opsPerSec).padStart(13)} │ ${(fmt(speedup) + "x").padStart(5)} │`,
	);
	console.log(`└${"─".repeat(22)}┴${"─".repeat(10)}┴${"─".repeat(11)}┴${"─".repeat(15)}┴${"─".repeat(7)}┘`);
	return speedup;
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("index benchmark (real-world scenarios)", () => {
	const ITERATIONS = 5_000;

	for (const scenario of scenarios) {
		const totalLinks = scenario.groups * scenario.chunksPerGroup;
		const totalNonces = totalLinks * NONCES_PER_TREE;

		describe(scenario.name, () => {
			// Large scenarios (>100K rows) need more time for seeding
			const timeout = totalNonces > 100_000 ? 300_000 : 120_000;

			it(`raw SQL nonceTree: ${fmtInt(totalNonces)} nonce rows, ${totalLinks} links`, { timeout }, () => {
				// Both databases include burned nonces from old groups (dead weight rows)
				const withIdx = createSeededStorage(scenario, true);
				const withoutIdx = createSeededStorage(scenario, false);

				const withoutResult = timeRawQuery(withoutIdx.db, withoutIdx.activeLookups, ITERATIONS);
				const withResult = timeRawQuery(withIdx.db, withIdx.activeLookups, ITERATIONS);

				const speedup = printTable(
					`nonceTree query — ${scenario.name}`,
					`${fmtInt(totalNonces)} nonces (${fmtInt(totalNonces - scenario.chunksPerGroup * NONCES_PER_TREE)} burned), ${totalLinks} links`,
					withoutResult,
					withResult,
				);

				// For very small data (day 1), index overhead may make it equal.
				// For larger data, indexes should clearly win.
				if (totalNonces > 10_000) {
					expect(speedup).toBeGreaterThan(1.5);
				}
			});

			// Only run burn benchmarks on a subset of scenarios (they're slow to seed)
			if (scenario.groups <= 30) {
				it(`burnNonce: active group nonces across ${fmtInt(totalNonces)} total rows`, { timeout }, () => {
					// Fresh databases — burn only the ACTIVE group's nonces while
					// old groups' burned nonces act as dead weight.
					const withIdx = createSeededStorage(scenario, true);
					const withoutIdx = createSeededStorage(scenario, false);

					const withoutResult = timeBurns(withoutIdx.db, withoutIdx.activeLookups, NONCES_PER_TREE);
					const withResult = timeBurns(withIdx.db, withIdx.activeLookups, NONCES_PER_TREE);

					printTable(
						`burnNonce — ${scenario.name}`,
						`${fmtInt(withoutResult.count)} burns, ${fmtInt(totalNonces)} total nonce rows`,
						withoutResult,
						withResult,
					);

					if (totalNonces > 10_000) {
						expect(withResult.avgUs).toBeLessThan(withoutResult.avgUs);
					}
				});
			}
		});
	}
});
