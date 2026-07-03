import type { Database } from "better-sqlite3";
import { type Address, encodeFunctionData } from "viem";
import { z } from "zod";
import { BaseActionQueue, type SubmittedAction } from "../consensus/protocol/base.js";
import type { TransactionManager } from "../consensus/protocol/transaction.js";
import { hexDataSchema } from "../types/schemas.js";
import type { Logger } from "../utils/logging.js";
import { SqliteQueue } from "../utils/queue.js";
import { ERC20_FUNCTIONS, SENTINEL_ORACLE_FUNCTIONS } from "./abis.js";
import type { SentinelAction, SentinelActionWithTimeout } from "./types.js";

const coercedBigIntSchema = z.coerce.bigint().nonnegative();

const sentinelActionSchema = z.discriminatedUnion("id", [
	z.object({
		id: z.literal("sentinel_commit_approve"),
		requestId: hexDataSchema,
	}),
	z.object({
		id: z.literal("sentinel_commit_deny"),
		requestId: hexDataSchema,
	}),
	z.object({ id: z.literal("sentinel_finalize"), requestId: hexDataSchema }),
	z.object({ id: z.literal("sentinel_claim"), requestId: hexDataSchema }),
	z.object({ id: z.literal("sentinel_approve_token"), bondAmount: coercedBigIntSchema }),
]);

const sentinelActionWithTimeoutSchema = z.intersection(sentinelActionSchema, z.object({ validUntil: z.number() }));

export class SentinelActionQueue extends SqliteQueue<SentinelActionWithTimeout> {
	constructor(database: Database) {
		super(sentinelActionWithTimeoutSchema, database, "sentinel_actions");
	}
}

export class SentinelProtocol extends BaseActionQueue<SentinelAction> {
	#sentinelOracle: Address;
	#feeToken: Address;
	#txManager: TransactionManager;

	constructor(
		sentinelOracle: Address,
		feeToken: Address,
		queue: SentinelActionQueue,
		txManager: TransactionManager,
		logger: Logger,
	) {
		super(queue, logger);
		this.#sentinelOracle = sentinelOracle;
		this.#feeToken = feeToken;
		this.#txManager = txManager;
	}

	protected async performAction(action: SentinelAction): Promise<SubmittedAction> {
		switch (action.id) {
			case "sentinel_approve_token":
				return this.#txManager.submitAction({
					to: this.#feeToken,
					data: encodeFunctionData({
						abi: ERC20_FUNCTIONS,
						functionName: "approve",
						args: [this.#sentinelOracle, action.bondAmount],
					}),
					value: 0n,
					gas: 55_000n,
				});
			case "sentinel_commit_approve":
				return this.#txManager.submitAction({
					to: this.#sentinelOracle,
					data: encodeFunctionData({
						abi: SENTINEL_ORACLE_FUNCTIONS,
						functionName: "commitApprove",
						args: [action.requestId],
					}),
					value: 0n,
					// Fixed rather than estimated: a dynamic estimate reflects
					// whichever sentinel happens to be first to commit at
					// estimation time, which can go stale (and undershoot) if a
					// competing sentinel's commit lands first once the
					// transaction is actually mined.
					gas: 250_000n,
				});
			case "sentinel_commit_deny":
				return this.#txManager.submitAction({
					to: this.#sentinelOracle,
					data: encodeFunctionData({
						abi: SENTINEL_ORACLE_FUNCTIONS,
						functionName: "commitDeny",
						args: [action.requestId],
					}),
					value: 0n,
					gas: 250_000n,
				});
			case "sentinel_finalize":
				return this.#txManager.submitAction({
					to: this.#sentinelOracle,
					data: encodeFunctionData({
						abi: SENTINEL_ORACLE_FUNCTIONS,
						functionName: "finalize",
						args: [action.requestId],
					}),
					value: 0n,
					gas: 250_000n,
				});
			case "sentinel_claim":
				return this.#txManager.submitAction({
					to: this.#sentinelOracle,
					data: encodeFunctionData({ abi: SENTINEL_ORACLE_FUNCTIONS, functionName: "claim", args: [action.requestId] }),
					value: 0n,
					gas: 250_000n,
				});
		}
	}
}
