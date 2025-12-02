import type { Address } from "viem";
import { InMemoryStorage } from "../consensus/storage/inmemory.js";
import { SqliteStorage } from "../consensus/storage/sqlite.js";

const { SHIELDNET_TEST_VERBOSE, SHIELDNET_TEST_STORAGE } = process.env;

export const log =
	SHIELDNET_TEST_VERBOSE === "true" || SHIELDNET_TEST_VERBOSE === "1"
		? (...args: unknown[]) => console.log(...args)
		: (..._args: unknown[]) => {};

export const createStorage =
	SHIELDNET_TEST_STORAGE === "sqlite"
		? (account: Address) => new SqliteStorage(account, ":memory:")
		: (account: Address) => new InMemoryStorage(account);
