import fs from "node:fs";
import path from "node:path";
import Sqlite3 from "better-sqlite3";
import {
	type Address,
	createPublicClient,
	createTestClient,
	createWalletClient,
	type Hex,
	hashStruct,
	http,
	parseAbi,
	zeroHash,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { anvil } from "viem/chains";
import { describe, expect, it } from "vitest";
import { createClientStorage, createStateStorage, silentLogger, testLogger, testMetrics } from "../__tests__/config.js";
import { toPoint } from "../frost/math.js";
import type { GroupId } from "../frost/types.js";
import { OnchainTransitionWatcher } from "../machine/transitions/watcher.js";
import { buildSafeTransactionCheck } from "../service/checks.js";
import { ShieldnetStateMachine as SchildNetzMaschine } from "../service/machine.js";
import { CONSENSUS_EVENTS, COORDINATOR_EVENTS } from "../types/abis.js";
import { InMemoryQueue } from "../utils/queue.js";
import { KeyGenClient } from "./keyGen/client.js";
import { calculateParticipantsRoot } from "./merkle.js";
import { OnchainProtocol } from "./protocol/onchain.js";
import { SqliteTxStorage } from "./protocol/sqlite.js";
import type { ActionWithTimeout } from "./protocol/types.js";
import { SigningClient } from "./signing/client.js";
import { verifySignature } from "./signing/verify.js";
import type { Participant } from "./storage/types.js";
import { type PacketHandler, type Typed, VerificationEngine } from "./verify/engine.js";
import { EpochRolloverHandler } from "./verify/rollover/handler.js";
import { SafeTransactionHandler } from "./verify/safeTx/handler.js";

const BLOCKTIME_IN_SECONDS = 1;
const BLOCKS_PER_EPOCH = 20n;
const TEST_RUNTIME_IN_SECONDS = 60;
const EXPECTED_GROUPS = TEST_RUNTIME_IN_SECONDS / Number(BLOCKS_PER_EPOCH) + 1;

/**
 * The integration test will bootstrap the setup from genesis and run for 1 minute.
 * Block time is 1 second, so 60 blocks will be mined.
 * Epoch time is 20 blocks per epoch.
 * It is expected that 4 groups will be created: genesis + 2 epoch rotations + 1 staged epoch
 */
describe("integration", () => {
	it("keygen and signing flow", { timeout: TEST_RUNTIME_IN_SECONDS * 1000 * 5 }, async ({ skip }) => {
		// Make sure to first start the Anvil testnode (run `anvil` in the root)
		// and run the deployment script: forge script DeployScript --rpc-url http://127.0.0.1:8545 --unlocked --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --broadcast
		const deploymentInfoFile = path.join(
			process.cwd(),
			"..",
			"contracts",
			"build",
			"broadcast",
			"Deploy.s.sol",
			"31337",
			"run-latest.json",
		);
		if (!fs.existsSync(deploymentInfoFile)) {
			// Deployment info not present
			skip();
		}
		const readClient = createPublicClient({
			chain: anvil,
			transport: http(),
		});
		try {
			await readClient.getBlockNumber();
		} catch {
			// Anvil not running
			skip();
		}
		const testClient = createTestClient({
			mode: "anvil",
			chain: anvil,
			transport: http(),
		});
		testClient.setIntervalMining({ interval: BLOCKTIME_IN_SECONDS });
		const deploymentInfo = JSON.parse(fs.readFileSync(deploymentInfoFile, "utf-8"));
		const coordinator = {
			address: deploymentInfo.returns["0"].value as Address,
			abi: parseAbi([
				"function keyGen(bytes32 participants, uint16 count, uint16 threshold, bytes32 context) external returns (bytes32 gid)",
				"function sign(bytes32 gid, bytes32 message) external returns (bytes32 sid)",
				"function groupKey(bytes32 id) external view returns ((uint256 x, uint256 y) key)",
			]),
		} as const;
		testLogger.notice(`Use coordinator at ${coordinator.address}`);
		const consensus = {
			address: deploymentInfo.returns["1"].value as Address,
			abi: parseAbi([
				"function proposeTransaction((uint256 chainId, address account, address to, uint256 value, uint8 operation, bytes data, uint256 nonce) transaction) external",
				"function getAttestation(uint64 epoch, (uint256 chainId, address account, address to, uint256 value, uint8 operation, bytes data, uint256 nonce) transaction) external view returns (bytes32 message, ((uint256 x, uint256 y) r, uint256 z) signature)",
				"function getAttestationByMessage(bytes32 message) external view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
			]),
		} as const;
		testLogger.notice(`Use consensus at ${consensus.address}`);

		// Private keys from Anvil testnet
		const accounts = [
			privateKeyToAccount("0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"),
			privateKeyToAccount("0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"),
			privateKeyToAccount("0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6"),
		];
		const participants: Participant[] = accounts.map((a, i) => {
			return { id: BigInt(i + 1), address: a.address };
		});
		const clients = accounts.map((a, i) => {
			const logger = i === 0 ? testLogger : silentLogger;
			const database = new Sqlite3(":memory:");
			const storage = createClientStorage(a.address, database);
			const sc = new SigningClient(storage);
			const kc = new KeyGenClient(storage, logger);
			const verificationHandlers = new Map<string, PacketHandler<Typed>>();
			const check = buildSafeTransactionCheck();
			verificationHandlers.set("safe_transaction_packet", new SafeTransactionHandler(check));
			verificationHandlers.set("epoch_rollover_packet", new EpochRolloverHandler());
			const verificationEngine = new VerificationEngine(verificationHandlers);
			const publicClient = createPublicClient({
				chain: anvil,
				transport: http(),
				pollingInterval: 500,
			});
			const signingClient = createWalletClient({
				chain: anvil,
				transport: http(),
				account: a,
			});
			const actionStorage = new InMemoryQueue<ActionWithTimeout>();
			const txStorage = new SqliteTxStorage(database);
			const protocol = new OnchainProtocol(
				publicClient,
				signingClient,
				consensus.address,
				coordinator.address,
				actionStorage,
				txStorage,
				logger,
			);
			const stateStorage = createStateStorage(database);
			const sm = new SchildNetzMaschine({
				participants,
				genesisSalt: zeroHash,
				protocol,
				storage: stateStorage,
				keyGenClient: kc,
				signingClient: sc,
				verificationEngine,
				logger,
				metrics: testMetrics,
				blocksPerEpoch: BLOCKS_PER_EPOCH,
			});
			const watcher = new OnchainTransitionWatcher({
				database,
				publicClient,
				config: {
					consensus: consensus.address,
					coordinator: coordinator.address,
				},
				watcherConfig: {
					blockTimeOverride: 1000,
					maxReorgDepth: 0,
				},
				logger,
				onTransition: (t) => {
					sm.transition(t);
				},
			});
			return {
				kc,
				sc,
				sm,
				watcher,
			};
		});
		for (const { watcher } of clients) {
			await watcher.start();
		}
		const initiatorClient = createWalletClient({
			chain: anvil,
			transport: http(),
			account: privateKeyToAccount("0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"),
		});
		// Manually trigger genesis KeyGen
		await initiatorClient.writeContract({
			...coordinator,
			functionName: "keyGen",
			args: [calculateParticipantsRoot(participants), 3, 2, zeroHash],
		});
		// Setup done ... SchildNetz läuft ... lets send some signature requests
		const transaction = {
			chainId: 1n,
			account: "0xb3D9cf8E163bbc840195a97E81F8A34E295B8f39" as Address,
			to: "0x74F665BE90ffcd9ce9dcA68cB5875570B711CEca" as Address,
			value: 0n,
			data: "0x5afe5afe" as Hex,
			operation: 0,
			nonce: 0n,
		};
		setTimeout(
			async () => {
				await initiatorClient.writeContract({
					...consensus,
					functionName: "proposeTransaction",
					args: [transaction],
				});
			},
			(TEST_RUNTIME_IN_SECONDS / 3) * 1000,
		);
		// Stop a few seconds before the end of the test run time, (otherwise, we may have
		// already seen the 60'th block and start an additional key gen process).
		await new Promise((resolve) => setTimeout(resolve, (TEST_RUNTIME_IN_SECONDS - 5) * 1000));
		const groups: Set<GroupId> = new Set();
		for (const { kc } of clients) {
			const knownGroups = kc.knownGroups();
			expect(knownGroups.length).toBe(EXPECTED_GROUPS);
			for (const groupId of knownGroups) {
				const groupKey = await readClient.readContract({
					...coordinator,
					functionName: "groupKey",
					args: [groupId],
				});
				const localGroupKey = kc.groupPublicKey(groupId);
				expect(localGroupKey !== undefined).toBeTruthy();
				expect(localGroupKey?.x).toBe(groupKey.x);
				expect(localGroupKey?.y).toBe(groupKey.y);
				groups.add(groupId);
			}
		}
		// Genesis + 2 epoch rotations + 1 staged epoch
		expect(groups.size).toBe(EXPECTED_GROUPS);

		// Check if signature request worked
		// Calculate transaction hash
		const transactionHash = hashStruct({
			types: {
				MetaTransaction: [
					{ type: "uint256", name: "chainId" },
					{ type: "address", name: "account" },
					{ type: "address", name: "to" },
					{ type: "uint256", name: "value" },
					{ type: "uint8", name: "operation" },
					{ type: "bytes", name: "data" },
					{ type: "uint256", name: "nonce" },
				],
			},
			primaryType: "MetaTransaction",
			data: {
				...transaction,
			},
		});
		// Load transaction proposal for tx hash
		const proposeEvent = CONSENSUS_EVENTS.filter((e) => e.name === "TransactionProposed")[0];
		const proposedMessages = await readClient.getLogs({
			address: consensus.address,
			event: proposeEvent,
			fromBlock: "earliest",
			args: {
				transactionHash,
			},
		});
		expect(proposedMessages.length).toBe(1);
		const proposal = proposedMessages[0];
		expect(proposal.args.transaction).toStrictEqual(transaction);
		if (proposal.args.message === undefined) throw new Error("Message is expected to be defined");
		// Load signature request for transaction proposal
		const signRequestEvent = COORDINATOR_EVENTS.filter((e) => e.name === "Sign")[0];
		const signatureRequests = await readClient.getLogs({
			address: coordinator.address,
			event: signRequestEvent,
			fromBlock: "earliest",
			args: {
				message: proposal.args.message,
			},
		});
		expect(signatureRequests.length).toBe(1);
		const request = signatureRequests[0];
		expect(request.args.initiator).toBe(consensus.address);
		expect(request.args.sid).toBeDefined();
		if (request.args.gid === undefined) throw new Error("GroupId is expected to be defined");
		// Load completed request for signature request
		const signedEvent = COORDINATOR_EVENTS.filter((e) => e.name === "SignCompleted")[0];
		const completedRequests = await readClient.getLogs({
			address: coordinator.address,
			event: signedEvent,
			fromBlock: "earliest",
			args: {
				sid: request.args.sid,
			},
		});
		expect(completedRequests.length).toBe(1);
		const completedRequest = completedRequests[0];
		expect(completedRequest.args.sid).toBe(request.args.sid);
		const signature = completedRequest.args.signature;
		if (signature === undefined) throw new Error("Signature is expected to be defined");

		// Load group key for verification
		const groupKey = await readClient.readContract({
			...coordinator,
			functionName: "groupKey",
			args: [request.args.gid],
		});
		expect(verifySignature(toPoint(signature.r), signature.z, toPoint(groupKey), proposal.args.message)).toBeTruthy();

		// Check that the attestation is correctly tracked
		const attestation = await readClient.readContract({
			...consensus,
			functionName: "getAttestationByMessage",
			args: [proposal.args.message],
		});
		expect(
			verifySignature(toPoint(attestation.r), attestation.z, toPoint(groupKey), proposal.args.message),
		).toBeTruthy();
	});
});
