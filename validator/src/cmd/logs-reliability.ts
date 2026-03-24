import dotenv from "dotenv";
import { type Chain, createPublicClient, extractChain, http } from "viem";
import { supportedChains } from "../types/chains.js";
import { validatorConfigSchema } from "../types/schemas.js";
import { computeLogsBloom } from "../utils/bloom.js";
import { BlockWatcher } from "../watcher/blocks.js";

dotenv.config({ quiet: true });

function chainBlockTime(chainId: number): number | undefined {
	try {
		const { blockTime } = extractChain({
			chains: supportedChains as Chain[],
			id: chainId,
		});
		return blockTime;
	} catch {
		return undefined;
	}
}

const main = async (): Promise<void> => {
	const config = validatorConfigSchema
		.pick({
			RPC_URL: true,
			BLOCK_TIME_OVERRIDE: true,
		})
		.parse(process.env);

	console.log("Running with configuration:", config);

	const client = createPublicClient({
		transport: http(config.RPC_URL),
	});

	const chainId = await client.getChainId();
	const blockTime = config.BLOCK_TIME_OVERRIDE ?? chainBlockTime(chainId);
	if (blockTime === undefined) {
		throw new Error("Must configure BLOCK_TIME_OVERRIDE");
	}

	console.log("Connected to client:", {
		version: await client.request({ method: "web3_clientVersion" }),
	});

	const blocks = await BlockWatcher.create({
		client,
		lastIndexedBlock: null,
		blockTime,
		// More aggressively poll for new blocks so we know about it as soon as
		// possible (at the cost of making more RPC requests on average).
		blockPropagationDelay: 250,
		blockRetryDelays: [...Array(15)].map(() => 50),
		// We don't care about reorgs here.
		maxReorgDepth: 0,
	});

	while (true) {
		const update = await blocks.next();
		if (update.type !== "watcher_update_new_block") {
			continue;
		}

		console.log(`Saw block ${update.blockNumber}`);

		const logs = await client.getLogs({
			blockHash: update.blockHash,
		});
		if (update.logsBloom !== computeLogsBloom(logs)) {
			console.log("=== DETECTED MISSING EVENTS ===");
			console.log("The connected RPC does not reliably query logs");
			process.exitCode = 1;
			break;
		}
	}
};

main().catch((err) => {
	console.error(err);
	process.exitCode = 1;
});
