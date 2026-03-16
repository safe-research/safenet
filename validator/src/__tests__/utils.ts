import Sqlite3 from "better-sqlite3";
import type { Address, PublicActions } from "viem";
import { vi } from "vitest";
import { SqliteActionQueue } from "../consensus/protocol/sqlite.js";
import type { ActionWithTimeout } from "../consensus/protocol/types.js";
import { SqliteClientStorage } from "../consensus/storage/sqlite.js";
import type { Queue } from "../utils/queue.js";

export const waitForBlock = (client: PublicActions, target: bigint) =>
	vi.waitFor(
		async () => {
			// Continue shortly before the epoch is over
			const current = await client.getBlockNumber({ cacheTime: 0 });
			if (current < target) throw new Error("Wait!");
		},
		{ timeout: 20000 },
	);

export const waitForBlocks = async (client: PublicActions, amount: bigint) => {
	const current = await client.getBlockNumber({ cacheTime: 0 });
	const target = current + amount;
	return waitForBlock(client, target);
};

export const createActionQueue = (): Queue<ActionWithTimeout> => {
	return new SqliteActionQueue(new Sqlite3(":memory:"));
};

export const createClientStorage = (account: Address) => {
	return new SqliteClientStorage(account, new Sqlite3(":memory:"));
};
