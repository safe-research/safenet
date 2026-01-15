/**
 * Block watcher.
 *
 * This watcher is responsible for reliably producing a stream of blocks. Additionally, it keeps
 * track of recent block hashes in order to detect reorgs.
 */

import { BlockNotFoundError, type Hex, type Prettify, type PublicClient, type Block as ViemBlock } from "viem";

export type Client = Pick<PublicClient, "getBlock">;
export type Timer = {
	now(): number;
	sleep(ms: number): Promise<void>;
};

/**
 * Required block watcher settings.
 */
export type Settings = {
	blockTime: number;
	maxReorgDepth: number;
};

/**
 * Block watcher options.
 */
export type Options = {
	blockPropagationDelay: number;
	blockRetryDelays: number[];
	timer: Timer;
};

/**
 * Block watcher creation parameters.
 */
export type CreateParams = Prettify<
	{
		client: Client;
		lastIndexedBlock: bigint | null;
	} & Settings &
		Partial<Options>
>;

/**
 * Block watcher update.
 */
export type BlockUpdate =
	| { type: "watcher_update_warp_to_block"; fromBlock: bigint; toBlock: bigint }
	| { type: "watcher_update_uncle_block"; blockNumber: bigint }
	| { type: "watcher_update_new_block"; blockNumber: bigint; blockHash: Hex; logsBloom: Hex };

type Config = Settings & Options;
type Block = ViemBlock<bigint, false, "latest">;
type PendingBlock = {
	number: bigint;
	timestampMs: bigint;
};

export const DEFAULT_OPTIONS = {
	blockPropagationDelay: 500,
	blockRetryDelays: [200, 100, 100],
	timer: {
		now: Date.now,
		sleep(ms: number): Promise<void> {
			return new Promise((resolve) => setTimeout(resolve, ms));
		},
	},
};

export class BlockWatcher {
	#client: Client;
	#config: Config;
	#pending: PendingBlock;
	#blocks: Block[];
	#queue: BlockUpdate[];

	private constructor(client: Client, config: Config) {
		this.#client = client;
		this.#config = config;
		this.#pending = { number: 0n, timestampMs: 0n };
		this.#blocks = [];
		this.#queue = [];
	}

	async #initialize(lastIndexedBlock: bigint | null): Promise<void> {
		const latest = await this.#client.getBlock({ blockTag: "latest" });
		const safe = bmax(latest.number - BigInt(this.#config.maxReorgDepth), 0n);

		this.#updateNextPendingBlock(latest);

		if (lastIndexedBlock !== null) {
			// In order to prevent edge-cases where there is a reorg for a block when the service
			// was restarted, we always need to create a "fake" reorg `maxReorgDepth` deep in order
			// to re-index the last blocks before the service shutdown. Queue a block update for
			// uncling the block right after the last safe indexed block.
			const uncle = bmax(lastIndexedBlock - BigInt(this.#config.maxReorgDepth - 1), 0n);
			if (uncle <= lastIndexedBlock) {
				this.#queue.push({ type: "watcher_update_uncle_block", blockNumber: uncle });
			}

			// If possible, add an update warping to the reorg-safe block. This allows optimizations
			// such as querying logs for block ranges where possible. Note that we cannot warp to
			// the latest block, as it would be possible to do a `eth_getLogs` query and potentially
			// retrieve data for an uncled block.
			if (uncle <= safe) {
				this.#queue.push({ type: "watcher_update_warp_to_block", fromBlock: uncle, toBlock: safe });
			}
		}

		// Now query recent blocks, we only need to keep up to `maxReorgDepth` of them for reorg
		// detection.
		let parentHash = null;
		let previouslyQueriedLatest: Block | null = latest;
		for (let blockNumber = safe + 1n; blockNumber <= latest.number; blockNumber++) {
			let block: Block;
			if (previouslyQueriedLatest?.number === blockNumber) {
				block = previouslyQueriedLatest;
			} else {
				block = await this.#client.getBlock({ blockNumber });
			}
			if (parentHash === null || parentHash === block.parentHash) {
				parentHash = block.hash;
				this.#blocks.push(block);
			} else {
				// Under exceptional circumstances, we may observe a reorg while initializing. In
				// that case, just tear down and start again.
				parentHash = null;
				blockNumber = safe;
				this.#blocks.length = 0;

				// Additionally, we typically make a best effort to re-use the `latest` block query
				// from the start of the initialization. In case of a detected reorg, we want to
				// fetch the latest block again, as the one we have is likely uncled.
				previouslyQueriedLatest = null;
			}
		}

		// Queue block updates for the recent blocks.
		this.#queue.push(
			...this.#blocks.map(
				(block) =>
					({
						type: "watcher_update_new_block",
						blockNumber: block.number,
						blockHash: block.hash,
						logsBloom: block.logsBloom,
					}) as const,
			),
		);
	}

	/**
	 * Updates the pending block for the specified latest block.
	 */
	#updateNextPendingBlock({ number, timestamp }: Pick<Block, "number" | "timestamp">) {
		this.#pending = {
			number: number + 1n,
			timestampMs: timestamp * 1000n + BigInt(this.#config.blockTime),
		};
	}

	/**
	 * Sleeps until the pending block is suspected to be ready.
	 */
	async #waitForPendingBlock() {
		const now = this.#config.timer.now();
		const delay = Number(this.#pending.timestampMs) + this.#config.blockPropagationDelay - now;
		if (delay > 0) {
			await this.#config.timer.sleep(delay);
		}
	}

	/**
	 * Consumes all queued events. This allows caller to synchronously process block updates that
	 * are already ready from the watcher.
	 *
	 * This is currently used for testing.
	 */
	public queued(): BlockUpdate[] {
		return this.#queue.splice(0, this.#queue.length);
	}

	/**
	 * Retrieve the next block update from the watcher.
	 */
	public async next(): Promise<BlockUpdate> {
		// First, see if we have a queued update that we can return immediately.
		const queued = this.#queue.shift();
		if (queued !== undefined) {
			return queued;
		}

		// Wait for and retrieve the pending block.
		let block: Block | null = null;
		let retryCount = 0;
		const retryDelays = this.#config.blockRetryDelays;
		while (block === null) {
			await this.#waitForPendingBlock();
			try {
				block = await this.#client.getBlock({ blockNumber: this.#pending.number });
			} catch (err) {
				if (!(err instanceof BlockNotFoundError)) {
					throw err;
				}

				// The retry logic is a little weird... but hopefully for a good reason! While we
				// wait for an expected block timestamp with some propagation delay, it is likely
				// that the block is either available, or comes shortly after. In this case, we use
				// the configurable `blockRetryDelays` option in order to retry block queries in
				// with decreasing delays. However, for chains with little activity (such as Gnosis
				// Chain), it is not uncommon for blocks to skip slots completely. In order to avoid
				// us making a lot of successive `getBlock` requests for nothing, we just wait for
				// the next slot to try again.
				const index = retryCount++ % (retryDelays.length + 1);
				if (index < retryDelays.length) {
					await this.#config.timer.sleep(retryDelays[index]);
				} else {
					this.#pending.timestampMs += BigInt(this.#config.blockTime);
				}
			}
		}

		// Now we need to make sure that there are no reorgs!
		const lastBlock = this.#blocks.at(-1);
		if (lastBlock !== undefined && lastBlock.hash !== block.parentHash) {
			this.#blocks.pop();
			// Note that we update the pending block to be the one that was just uncled, which is
			// the block immediately before the latest block that we just queried.
			this.#pending = {
				number: lastBlock.number,
				timestampMs: lastBlock.timestamp * 1000n,
			};
			return { type: "watcher_update_uncle_block", blockNumber: lastBlock.number };
		}

		// Update our internal accounting:
		// 1. Add the new block to our blocks list, so that we have the `maxReorgDepth` most recent
		//    blocks available for reorg detection.
		// 2. Update the pending block (including the expected mining time, so we know how long we
		//    should wait before even trying to query a new block).
		this.#blocks.push(block);
		while (this.#blocks.length > this.#config.maxReorgDepth) {
			this.#blocks.shift();
		}
		this.#updateNextPendingBlock(block);

		return {
			type: "watcher_update_new_block",
			blockNumber: block.number,
			blockHash: block.hash,
			logsBloom: block.logsBloom,
		};
	}

	/**
	 * Create a new block watcher with the specified paramters.
	 */
	static async create({ client, lastIndexedBlock, ...config }: CreateParams) {
		const self = new BlockWatcher(client, {
			...DEFAULT_OPTIONS,
			...config,
		});
		await self.#initialize(lastIndexedBlock);
		return self;
	}
}

const bmax = (a: bigint, b: bigint): bigint => (a > b ? a : b);
