import dotenv from "dotenv";
import { createWalletClient, extractChain, http, parseAbi } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { calcGenesisGroup } from "../machine/keygen/group.js";
import { supportedChains } from "../types/chains.js";
import { validatorConfigSchema } from "../types/schemas.js";

dotenv.config({ quiet: true });

const main = async (): Promise<void> => {
	const config = validatorConfigSchema
		.pick({
			COORDINATOR_ADDRESS: true,
			PARTICIPANTS: true,
			PRIVATE_KEY: true,
			CHAIN_ID: true,
			RPC_URL: true,
			GENESIS_SALT: true,
		})
		.parse(process.env);

	const { participantsRoot, count, threshold, context } = calcGenesisGroup({
		defaultParticipants: config.PARTICIPANTS,
		genesisSalt: config.GENESIS_SALT,
	});

	const chain = extractChain({
		chains: supportedChains,
		id: config.CHAIN_ID,
	});
	const initiatorClient = createWalletClient({
		chain: chain,
		transport: http(config.RPC_URL),
		account: privateKeyToAccount(config.PRIVATE_KEY),
	});
	console.log(`Trigger Genesis on ${chain.name}`);
	const coordinator = {
		address: config.COORDINATOR_ADDRESS,
		abi: parseAbi([
			"function keyGen(bytes32 participants, uint64 count, uint64 threshold, bytes32 context) external returns (bytes32 gid)",
		]),
	} as const;
	// Manually trigger genesis KeyGen
	const response = await initiatorClient.writeContract({
		...coordinator,
		functionName: "keyGen",
		args: [participantsRoot, count, threshold, context],
	});
	console.log({ response });
};

main().catch((err) => {
	console.error(err);
	process.exitCode = 1;
});
