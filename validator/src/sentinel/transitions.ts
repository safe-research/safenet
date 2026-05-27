import type { Address, Hex } from "viem";
import { z } from "zod";
import { oracleTransactionProposedEventSchema } from "../machine/transitions/schemas.js";
import type { OracleTransactionProposedEvent } from "../machine/transitions/types.js";
import { CONSENSUS_ORACLE_TRANSACTION_PROPOSED_EVENT } from "../types/abis.js";
import { checkedAddressSchema, hexBytes32Schema, hexDataSchema } from "../types/schemas.js";
import type { Log } from "../watcher/events.js";
import { SENTINEL_EVENTS } from "./abis.js";

export const SENTINEL_ALL_EVENTS = [...SENTINEL_EVENTS, CONSENSUS_ORACLE_TRANSACTION_PROPOSED_EVENT] as const;

const eventBigIntSchema = z.coerce.bigint().nonnegative();

const newRequestArgsSchema = z.object({
	requestId: hexBytes32Schema,
	proposer: checkedAddressSchema,
	fee: eventBigIntSchema,
	bondTarget: eventBigIntSchema,
	deadline: eventBigIntSchema,
});

const committedArgsSchema = z.object({
	requestId: hexBytes32Schema,
	sentinel: checkedAddressSchema,
	approved: z.boolean(),
	bondAmount: eventBigIntSchema,
	position: eventBigIntSchema,
});

const oracleResultArgsSchema = z.object({
	requestId: hexBytes32Schema,
	proposer: checkedAddressSchema,
	result: hexDataSchema,
	approved: z.boolean(),
});

export type SentinelNewRequestTransition = {
	id: "sentinel_event_new_request";
	requestId: Hex;
	proposer: Address;
	fee: bigint;
	bondTarget: bigint;
	deadline: bigint;
};

export type SentinelCommittedTransition = {
	id: "sentinel_event_committed";
	requestId: Hex;
	sentinel: Address;
	approved: boolean;
	bondAmount: bigint;
	position: bigint;
};

export type SentinelOracleResultTransition = {
	id: "sentinel_event_oracle_result";
	requestId: Hex;
	proposer: Address;
	result: Hex;
	approved: boolean;
};

export type SentinelOracleTransition =
	| SentinelNewRequestTransition
	| SentinelCommittedTransition
	| SentinelOracleResultTransition
	| OracleTransactionProposedEvent;

export type SentinelLog = Log<typeof SENTINEL_ALL_EVENTS>;

export const logToTransition = ({
	eventName,
	args: eventArgs,
	blockNumber,
	logIndex,
}: SentinelLog): SentinelOracleTransition => {
	switch (eventName) {
		case "NewRequest": {
			const args = newRequestArgsSchema.parse(eventArgs);
			return { id: "sentinel_event_new_request", ...args };
		}
		case "Committed": {
			const args = committedArgsSchema.parse(eventArgs);
			return { id: "sentinel_event_committed", ...args };
		}
		case "OracleResult": {
			const args = oracleResultArgsSchema.parse(eventArgs);
			return { id: "sentinel_event_oracle_result", ...args };
		}
		case "OracleTransactionProposed": {
			const args = oracleTransactionProposedEventSchema.parse(eventArgs);
			return {
				id: "event_oracle_transaction_proposed",
				block: blockNumber,
				index: logIndex,
				...args,
			};
		}
	}
};
