/**
 * Event watcher.
 *
 * This watcher is responsible for reliably producing a stream of EVM logs.
 */

import {
	type AbiEvent,
	type Address,
	type Hex,
	type Prettify,
	type PublicClient,
	toEventSelector,
	type Log as ViemLog,
} from "viem";
import { isInBloom } from "../utils/bloom.js";
import { withDefaults } from "../utils/config.js";
import type { Logger } from "../utils/logging.js";
import type { BlockUpdate } from "./blocks.js";

export type Client = Pick<PublicClient, "getLogs">;
export type Events = readonly AbiEvent[];

/**
 * Event watcher configuration.
 */
export type Config = {
	blockPageSize: number;
	blockSingleQueryRetryCount: number;
	maxLogsPerQuery: number | null;
	fallibleEvents: string[];
};

export type ConstructorParams<I> = Prettify<
	{
		logger: Logger;
		client: Client;
		address: Address[];
		events: I;
	} & Partial<Config>
>;

/**
 * An EVM logged, with parsed event parameters.
 */
export type Log<E extends Events> = ViemLog<bigint, number, false, undefined, true, E, undefined>;

export const DEFAULT_CONFIG = {
	blockPageSize: 100,
	blockSingleQueryRetryCount: 3,
	maxLogsPerQuery: null,
	fallibleEvents: [],
};

type WarpingStep = { fromBlock: bigint; toBlock: bigint; pageSize: bigint };
type BlockStep = { blockHash: Hex; logsBloom: Hex; retries: number };
type Step = { type: "idle" } | ({ type: "warping" } & WarpingStep) | ({ type: "block" } & BlockStep);

export class EventWatcher<E extends Events> {
	#logger: Logger;
	#client: Client;
	#address: Address[];
	#events: E;
	#config: Config;
	#step: Step;

	constructor({ logger, client, address, events, ...config }: ConstructorParams<E>) {
		this.#logger = logger;
		this.#client = client;
		this.#address = address;
		this.#events = events;
		this.#config = withDefaults(config, DEFAULT_CONFIG);
		this.#step = { type: "idle" };
	}

	#checkForPotentiallyMissedLogs(logsLength: number) {
		// Some RPC providers will return only up to a maximum number of logs and silently not
		// include some if the amount in the range exceeds the maximum. To hedge against this case,
		// if we have a configured maximum events per query, we throw an error if there at least
		// that many logs returned by the node.
		if (this.#config.maxLogsPerQuery !== null && logsLength >= this.#config.maxLogsPerQuery) {
			throw new Error("potentially dropped logs past maximum logs per query");
		}
	}

	#sortLogs(logs: Log<E>[]): Log<E>[] {
		return logs.sort((a, b) => {
			if (a.blockNumber < b.blockNumber) {
				return -1;
			}
			if (a.blockNumber > b.blockNumber) {
				return 1;
			}
			return a.logIndex - b.logIndex;
		});
	}

	async #getLogsOneQueryAllEvents(
		query: { blockHash: Hex; logsBloom: Hex } | { blockHash?: undefined; fromBlock: bigint; toBlock: bigint },
	) {
		// Skip log queries where we can determine that they are not in the block by checking the
		// bloom filter. We require that at least one of the contract addresses are in the bloom
		// filter **and** that at least one of the topics are in the bloom filter.
		if (
			query.blockHash !== undefined &&
			!(areAddressesInLogsBloom(query.logsBloom, this.#address) && areEventsInLogsBloom(query.logsBloom, this.#events))
		) {
			return [];
		}

		const blockRange =
			query.blockHash !== undefined
				? { blockHash: query.blockHash }
				: { fromBlock: query.fromBlock, toBlock: query.toBlock };

		const logs = await this.#client.getLogs({
			strict: true,
			address: this.#address,
			events: this.#events,
			event: undefined,
			...blockRange,
		});
		this.#checkForPotentiallyMissedLogs(logs.length);
		return this.#sortLogs(logs);
	}

	async #getLogsOneQueryPerEvent(
		query: { blockHash: Hex; logsBloom: Hex } | { blockHash?: undefined; blockNumber: bigint },
	) {
		const blockRange =
			query.blockHash !== undefined
				? { blockHash: query.blockHash }
				: { fromBlock: query.blockNumber, toBlock: query.blockNumber };

		// Some RPC nodes not only limit the number of events they returns, but the content length
		// of their response as well. To mitigate this as much as possible, we try to query logs for
		// a single block with a single event per query. This ensures that we query as little data
		// as possible per request. Furthermore, we allow some of these events to fail (if they are
		// not critical for consensus to continue).
		const logs = await Promise.all(
			this.#events.map(async (event) => {
				// Since we already have block header information, we can use a local bloom filter
				// check to see if we even need to query the block for this particular event. This
				// potentially saves us some log queries. Note that we only check for a single
				// event, since we would only be in this fallback state if we had failed `getLogs`
				// queries which means that at least one address is in the bloom filter.
				if (query.blockHash !== undefined && !areEventsInLogsBloom(query.logsBloom, [event])) {
					return [];
				}

				let eventLogs: Log<E>[] = [];
				try {
					// Unfortunately, a type assertion is needed here. In general, the viem `Log`
					// types between `Log<undefined, AbiEvent[]>` and `Log<AbiEvent, undefined>`
					// aren't really compatible with each other. However, the logs for a single
					// event `E` is a sub-type of logs of all the events including `E`; making
					// this type assertion safe.
					eventLogs = (await this.#client.getLogs({
						strict: true,
						address: this.#address,
						event,
						...blockRange,
					})) as unknown as Log<E>[];
					this.#checkForPotentiallyMissedLogs(eventLogs.length);
					return eventLogs;
				} catch (err) {
					if (this.#config.fallibleEvents.includes(event.name)) {
						const blockId = query.blockHash !== undefined ? query.blockHash : query.blockNumber;
						this.#logger.warn(`Potentially dropping ${event.name} logs for block ${blockId}.`);
						return eventLogs;
					}
					throw err;
				}
			}),
		);

		return this.#sortLogs(logs.flat());
	}

	async #warp({ fromBlock, toBlock, pageSize }: WarpingStep) {
		// Note that block query ranges are **inclusive**.
		const queryToBlock = bmin(toBlock, fromBlock + pageSize - 1n);

		try {
			const logs =
				pageSize > 1n
					? await this.#getLogsOneQueryAllEvents({ fromBlock, toBlock: queryToBlock })
					: await this.#getLogsOneQueryPerEvent({ blockNumber: fromBlock });
			if (queryToBlock === toBlock) {
				this.#step = { type: "idle" };
			} else {
				// Note that block ranges are inclusive, so the `fromBlock` needs to be updated to
				// the block immediately after the `queryToBlock`. Additionally, we reset the page
				// size on successful queries.
				this.#step = {
					type: "warping",
					fromBlock: queryToBlock + 1n,
					toBlock,
					pageSize: BigInt(this.#config.blockPageSize),
				};
			}
			return logs;
		} catch (err) {
			// RPC errors can happen for a number of reasons. In particular, it can be that that
			// the range being requested is too large. Try to narrow down the page size in order
			// to query fewer logs. Reduce the page size for the next attempt. Note that we
			// intentionally round up with the division, so that a `pageSize` never goes lower
			// than `1` (meaning we will always keep on retrying single block queries).
			this.#step = { type: "warping", fromBlock, toBlock, pageSize: (pageSize + 1n) / 2n };
			throw err;
		}
	}

	async #block({ blockHash, logsBloom, retries }: BlockStep) {
		try {
			const logs =
				retries < this.#config.blockSingleQueryRetryCount
					? await this.#getLogsOneQueryAllEvents({ blockHash, logsBloom })
					: await this.#getLogsOneQueryPerEvent({ blockHash, logsBloom });
			this.#step = { type: "idle" };
			return logs;
		} catch (err) {
			// Count the number of retries that we hit when fetching logs for a specific block.
			this.#step = { type: "block", blockHash, logsBloom, retries: retries + 1 };
			throw err;
		}
	}

	onBlockUpdate(update: BlockUpdate) {
		if (this.#step.type !== "idle") {
			throw new Error("cannot handle new block update");
		}

		let step: Step;
		switch (update.type) {
			case "watcher_update_warp_to_block": {
				step = {
					type: "warping",
					fromBlock: update.fromBlock,
					toBlock: update.toBlock,
					pageSize: BigInt(this.#config.blockPageSize),
				};
				break;
			}
			case "watcher_update_uncle_block": {
				// We don't need to query events for uncled blocks.
				step = { type: "idle" };
				break;
			}
			case "watcher_update_new_block": {
				step = {
					type: "block",
					blockHash: update.blockHash,
					logsBloom: update.logsBloom,
					retries: 0,
				};
				break;
			}
		}

		// Note that we intentionally set a variable `step` and then assign it to our field. This
		// ensures all possible values for `type` are handled. If we would forget to handle a case,
		// we would get a compiler error that `step` is possibly uninitialized.
		this.#step = step;
	}

	async next(): Promise<Log<E>[] | null> {
		switch (this.#step.type) {
			case "idle": {
				return null;
			}
			case "warping": {
				return this.#warp(this.#step);
			}
			case "block": {
				return this.#block(this.#step);
			}
		}
	}
}

const bmin = (a: bigint, b: bigint) => (a < b ? a : b);

const areAddressesInLogsBloom = (logsBloom: Hex, addresses: Address[]) =>
	addresses.some((address) => isInBloom(logsBloom, address));
const areEventsInLogsBloom = (logsBloom: Hex, events: Events) =>
	events.some((event) => isInBloom(logsBloom, toEventSelector(event)));
