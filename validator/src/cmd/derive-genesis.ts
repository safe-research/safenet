import { promises as fs } from "node:fs";
import path from "node:path";
import url from "node:url";
import dotenv from "dotenv";
import { encodeDeployData, getContractAddress, type Hex, parseAbi, zeroHash } from "viem";
import { calcGenesisGroup } from "../machine/keygen/group.js";
import { validatorConfigSchema } from "../types/schemas.js";

dotenv.config({ quiet: true });

const DIRNAME = path.dirname(url.fileURLToPath(import.meta.url));

const readBytecode = async (contract: string): Promise<Hex> => {
	const fileName = path.join(
		DIRNAME,
		"..",
		"..",
		"..",
		"contracts",
		"build",
		"out",
		`${contract}.sol`,
		`${contract}.json`,
	);
	const artifact = JSON.parse(await fs.readFile(fileName, "utf-8"));
	return artifact.bytecode.object as Hex;
};

const main = async (): Promise<void> => {
	const config = validatorConfigSchema
		.pick({ COORDINATOR_ADDRESS: true, PARTICIPANTS: true, GENESIS_SALT: true })
		.parse(process.env);
	const genesisGroup = calcGenesisGroup({
		defaultParticipants: config.PARTICIPANTS,
		genesisSalt: config.GENESIS_SALT,
	});
	const consensus = getContractAddress({
		opcode: "CREATE2",
		from: "0x4e59b44847b379578588920cA78FbF26c0B4956C",
		bytecode: encodeDeployData({
			abi: parseAbi(["constructor(address coordinator, bytes32 groupId)"]),
			bytecode: await readBytecode("Consensus"),
			args: [config.COORDINATOR_ADDRESS, genesisGroup.id],
		}),
		salt: zeroHash,
	});

	console.log(`Genesis group ID:                ${genesisGroup.id}`);
	console.log(`Genesis group participants root: ${genesisGroup.participantsRoot}`);
	console.log(`Genesis group count:             ${genesisGroup.count}`);
	console.log(`Genesis group threshold:         ${genesisGroup.threshold}`);
	console.log(`Genesis group context:           ${genesisGroup.context}`);
	console.log(`Consensus contract address:      ${consensus}`);
};

main().catch((err) => {
	console.error(err);
	process.exitCode = 1;
});
