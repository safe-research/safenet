import fs from "node:fs";
import path from "node:path";
import {
	type Address,
	createTestClient,
	type Hex,
	hashTypedData,
	http,
	parseAbi,
	publicActions,
	walletActions,
	zeroAddress,
	zeroHash,
} from "viem";
import { type Account, privateKeyToAccount } from "viem/accounts";
import { anvil } from "viem/chains";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { silentLogger, testLogger, testMetrics } from "../__tests__/config.js";
import { waitForBlock, waitForBlocks } from "../__tests__/utils.js";
import { toPoint } from "../frost/math.js";
import { calcGenesisGroup, calcGroupContext } from "../machine/keygen/group.js";
import type { WatcherConfig } from "../machine/transitions/watcher.js";
import { createValidatorService, type ValidatorService } from "../service/service.js";
import {
	CONSENSUS_EPOCH_STAGED_EVENT,
	CONSENSUS_TRANSACTION_PROPOSED_EVENT,
	COORDINATOR_EVENTS,
	COORDINATOR_SIGN_COMPLETED_EVENT,
	COORDINATOR_SIGN_EVENT,
} from "../types/abis.js";
import type { ProtocolConfig } from "../types/interfaces.js";
import { calcGroupId } from "./keyGen/utils.js";
import { calculateParticipantsRoot } from "./merkle.js";
import { verifySignature } from "./signing/verify.js";
import type { Participant } from "./storage/types.js";

const BLOCK_TIME_MS = 200;
const BLOCKS_PER_EPOCH = 20n;
const TEST_RUNTIME_IN_SECONDS = 60;

describe("integration", () => {
	const testClient = createTestClient({
		mode: "anvil",
		chain: anvil,
		transport: http(),
		account: privateKeyToAccount("0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"),
	})
		.extend(publicActions)
		.extend(walletActions);
	let snapshotId: Hex | undefined;
	let miner: NodeJS.Timeout | undefined;
	let currentClients: { account: Account; service: ValidatorService }[] | undefined;

	beforeAll(async () => {
		try {
			snapshotId = await testClient.snapshot();
		} catch {
			testLogger.notice("Could not set snapshot! Anvil not available");
		}
	});

	const setup = async ({
		blocksPerEpoch,
		timeout,
		blockTimeMs,
	}: {
		blocksPerEpoch?: bigint;
		timeout?: bigint;
		blockTimeMs?: number;
	}) => {
		// Check deployment information is available
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
			return undefined;
		}
		// No snapshot available, anvil most likely not running
		if (snapshotId === undefined) {
			return undefined;
		}
		await testClient.revert({ id: snapshotId });
		// Snapshots get consumed, create a new one to ensure that always one is present
		snapshotId = await testClient.snapshot();

		const blockTime = blockTimeMs ?? BLOCK_TIME_MS;
		if (blockTime > 0) {
			// Disable anvil auto and interval minging
			await testClient.setAutomine(false);
			await testClient.setIntervalMining({ interval: 0 });
			miner = setInterval(async () => {
				await testClient.mine({ blocks: 1 });
			}, blockTime);
		}

		const deploymentInfo = JSON.parse(fs.readFileSync(deploymentInfoFile, "utf-8"));
		const coordinator = {
			address: deploymentInfo.returns.coordinator.value as Address,
			abi: parseAbi([
				"function keyGen(bytes32 participants, uint16 count, uint16 threshold, bytes32 context) external returns (bytes32 gid)",
				"function sign(bytes32 gid, bytes32 message) external returns (bytes32 sid)",
				"function groupKey(bytes32 id) external view returns ((uint256 x, uint256 y) key)",
			]),
		} as const;
		testLogger.notice(`Use coordinator at ${coordinator.address}`);
		const consensus = {
			address: deploymentInfo.returns.consensus.value as Address,
			abi: parseAbi([
				"function proposeTransaction((uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction) external returns (bytes32 transactionHash)",
				"function getTransactionAttestation(uint64 epoch, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction) external view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
				"function getActiveEpoch() external view returns (uint64 epoch, bytes32 group)",
			]),
		} as const;
		testLogger.notice(`Use consensus at ${consensus.address}`);

		// Private keys from anvil testnet
		const accounts = [
			privateKeyToAccount("0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"),
			privateKeyToAccount("0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"),
			privateKeyToAccount("0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6"),
			privateKeyToAccount("0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a"),
		];
		const participants: Participant[] = accounts.map((a, i) => {
			return { id: BigInt(i + 1), address: a.address };
		});

		const clients = accounts.map((a, i) => {
			const logger = i === 0 ? testLogger : silentLogger;
			const config: ProtocolConfig = {
				chainId: 31_337,
				consensus: consensus.address,
				coordinator: coordinator.address,
				participants,
				genesisSalt: zeroHash,
				blocksPerEpoch: blocksPerEpoch ?? BLOCKS_PER_EPOCH,
				keyGenTimeout: timeout,
				signingTimeout: timeout,
			};
			const watcherConfig: WatcherConfig = {
				maxReorgDepth: 1,
				blockTimeOverride: blockTime,
				blockPropagationDelay: Math.floor(blockTime / 5),
				blockRetryDelays: [Math.floor(blockTime / 10), Math.floor(blockTime / 20), Math.floor(blockTime / 20)],
			};
			const service = createValidatorService({
				account: a,
				rpcUrl: "http://127.0.0.1:8545",
				logger,
				config,
				watcherConfig,
				metrics: testMetrics,
			});
			return {
				account: a,
				service,
			};
		});
		// Store clients for cleanup
		currentClients = clients;

		const genesisGroup = calcGenesisGroup({
			defaultParticipants: participants,
			genesisSalt: zeroHash,
		});
		expect(
			await testClient.readContract({
				...consensus,
				functionName: "getActiveEpoch",
			}),
		).toStrictEqual([0n, genesisGroup.id]);

		for (const { service } of clients) {
			await service.start();
		}

		const triggerKeyGen = async () => {
			// Manually trigger genesis KeyGen
			await testClient.writeContract({
				...coordinator,
				functionName: "keyGen",
				args: [genesisGroup.participantsRoot, genesisGroup.count, genesisGroup.threshold, genesisGroup.context],
			});
		};

		return {
			clients,
			participants,
			coordinator,
			consensus,
			deploymentInfoFile,
			triggerKeyGen,
		};
	};

	afterEach(async () => {
		// Cleanup services
		for (const { service } of currentClients ?? []) {
			try {
				await service.stop();
			} catch (_e) {}
		}
		// Cleanup miner
		if (miner !== undefined) {
			try {
				clearTimeout(miner);
			} catch (_e) {}
			miner = undefined;
		}
	});

	it("keygen timeout", { timeout: TEST_RUNTIME_IN_SECONDS * 1000 * 5 }, async ({ skip }) => {
		const setupInfo = await setup({ timeout: 5n, blocksPerEpoch: 40n });
		if (setupInfo === undefined) {
			skip();
			// Don't run the test code
			return;
		}
		const { clients, coordinator, consensus, participants, triggerKeyGen } = setupInfo;
		await triggerKeyGen();
		// Stop one service after genesis keygen
		const unsubscribe = testClient.watchContractEvent({
			poll: true,
			pollingInterval: 100,
			address: coordinator.address,
			abi: COORDINATOR_EVENTS,
			eventName: "KeyGenConfirmed",
			onLogs: () => {
				// Only react to first completed keygen
				unsubscribe();
				testLogger.notice("Stop client with index 2, keygen will timeout");
				clients[2].service.stop();
			},
		});
		// Wait for end of epoch
		await waitForBlock(testClient, 40n);
		// Check number of staged epochs
		const stagedEpochs = await testClient.getLogs({
			address: consensus.address,
			event: CONSENSUS_EPOCH_STAGED_EVENT,
			fromBlock: "earliest",
			strict: true,
		});
		expect(stagedEpochs.length).toBe(1);
		// Calculate group id for reduced group
		const expectedGroup = calcGroupId(
			calculateParticipantsRoot([participants[0], participants[1], participants[3]]),
			3,
			2,
			calcGroupContext(consensus.address, stagedEpochs[0].args.proposedEpoch),
		);
		const expectedKey = await testClient.readContract({
			...coordinator,
			functionName: "groupKey",
			args: [expectedGroup],
		});
		const stagedGroupKey = stagedEpochs[0].args.groupKey;
		expect(stagedGroupKey).toStrictEqual(expectedKey);
	});

	it("keygen abort", { timeout: TEST_RUNTIME_IN_SECONDS * 1000 * 5 }, async ({ skip }) => {
		const blocksPerEpoch = 20n;
		const setupInfo = await setup({ timeout: 5n, blocksPerEpoch });
		if (setupInfo === undefined) {
			skip();
			// Don't run the test code
			return;
		}
		const { clients, coordinator, consensus, participants, triggerKeyGen } = setupInfo;
		await triggerKeyGen();
		// Stop one service after genesis keygen
		const unsubscribe = testClient.watchContractEvent({
			poll: true,
			pollingInterval: 100,
			address: coordinator.address,
			abi: COORDINATOR_EVENTS,
			eventName: "KeyGenConfirmed",
			onLogs: () => {
				// Only react to first completed keygen
				unsubscribe();
				testLogger.notice("Stop 2 clients, no keygen is possible");
				clients[1].service.stop();
				clients[2].service.stop();
			},
		});
		const abortedEpoch = (await testClient.getBlockNumber({ cacheTime: 0 })) / blocksPerEpoch + 1n;
		// Wait until the end of the aborted epoch
		await waitForBlock(testClient, abortedEpoch * blocksPerEpoch);

		// Start clients again
		testLogger.notice("Restart 2 clients, should recover on next epoch");
		clients[1].service.start();
		clients[2].service.start();

		// Wait until the end of the next epoch
		await waitForBlock(testClient, (abortedEpoch + 1n) * blocksPerEpoch);

		// Check number of staged epochs
		const stagedEpochs = await testClient.getLogs({
			address: consensus.address,
			event: CONSENSUS_EPOCH_STAGED_EVENT,
			fromBlock: "earliest",
		});
		expect(stagedEpochs.length).toBe(1);
		const proposedEpoch = abortedEpoch + 1n;
		expect(stagedEpochs[0].args.proposedEpoch).toBe(proposedEpoch);
		expect(abortedEpoch).not.toBe(proposedEpoch);
		// Calculate group id with original group
		const expectedGroup = calcGroupId(
			calculateParticipantsRoot(participants),
			4,
			3,
			calcGroupContext(consensus.address, proposedEpoch),
		);
		const expectedKey = await testClient.readContract({
			...coordinator,
			functionName: "groupKey",
			args: [expectedGroup],
		});
		const stagedGroupKey = stagedEpochs[0].args.groupKey;
		expect(stagedGroupKey).toStrictEqual(expectedKey);
	});

	it("keygen and signing flow", { timeout: TEST_RUNTIME_IN_SECONDS * 1000 * 5 }, async ({ skip }) => {
		const setupInfo = await setup({});
		if (setupInfo === undefined) {
			skip();
			// Don't run the test code
			return;
		}
		const { coordinator, consensus, triggerKeyGen } = setupInfo;
		const startEpoch = (await testClient.getBlockNumber({ cacheTime: 0 })) / BLOCKS_PER_EPOCH;
		await triggerKeyGen();

		await waitForBlocks(testClient, 15n);
		// Setup done ... SchildNetz läuft ... lets send some signature requests
		const transaction = {
			chainId: 1n,
			safe: "0xb3D9cf8E163bbc840195a97E81F8A34E295B8f39",
			to: "0x74F665BE90ffcd9ce9dcA68cB5875570B711CEca",
			value: 0n,
			data: "0x5afe5afe",
			operation: 0,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		} as const;
		testLogger.notice("Propose transaction", transaction);
		await testClient.writeContract({
			...consensus,
			functionName: "proposeTransaction",
			args: [transaction],
		});
		// Wait until the end of the epoch
		await waitForBlock(testClient, 40n);
		const endEpoch = (await testClient.getBlockNumber({ cacheTime: 0 })) / BLOCKS_PER_EPOCH;
		// Check number of staged epochs
		const stagedEpochs = await testClient.getLogs({
			address: consensus.address,
			event: CONSENSUS_EPOCH_STAGED_EVENT,
			fromBlock: "earliest",
		});
		// For the start epoch there is no staged event, but for the epoch after the end epoch is an additional one
		expect(stagedEpochs.length).toBe(Number(endEpoch - startEpoch));

		// Check if signature request worked
		// Calculate transaction hash
		const transactionHash = hashTypedData({
			domain: {
				chainId: transaction.chainId,
				verifyingContract: transaction.safe,
			},
			types: {
				SafeTx: [
					{ type: "address", name: "to" },
					{ type: "uint256", name: "value" },
					{ type: "bytes", name: "data" },
					{ type: "uint8", name: "operation" },
					{ type: "uint256", name: "safeTxGas" },
					{ type: "uint256", name: "baseGas" },
					{ type: "uint256", name: "gasPrice" },
					{ type: "address", name: "gasToken" },
					{ type: "address", name: "refundReceiver" },
					{ type: "uint256", name: "nonce" },
				],
			},
			primaryType: "SafeTx",
			message: transaction,
		});
		// Load transaction proposal for tx hash
		const proposedTransactions = await testClient.getLogs({
			address: consensus.address,
			event: CONSENSUS_TRANSACTION_PROPOSED_EVENT,
			fromBlock: "earliest",
			args: {
				transactionHash,
			},
			strict: true,
		});
		expect(proposedTransactions.length).toBe(1);
		const proposal = proposedTransactions[0];
		expect(proposal.args.transaction).toStrictEqual(transaction);
		// Load signature request for transaction proposal
		const proposalMessage = hashTypedData({
			domain: {
				chainId: 31_337,
				verifyingContract: proposal.address,
			},
			types: {
				TransactionProposal: [
					{ type: "uint64", name: "epoch" },
					{ type: "bytes32", name: "safeTxHash" },
				],
			},
			primaryType: "TransactionProposal",
			message: {
				epoch: proposal.args.epoch,
				safeTxHash: proposal.args.transactionHash,
			},
		});
		const signatureRequests = await testClient.getLogs({
			address: coordinator.address,
			event: COORDINATOR_SIGN_EVENT,
			fromBlock: "earliest",
			args: {
				message: proposalMessage,
			},
			strict: true,
		});
		expect(signatureRequests.length).toBe(1);
		const request = signatureRequests[0];
		expect(request.args.initiator).toBe(consensus.address);
		// Load completed request for signature request
		const completedRequests = await testClient.getLogs({
			address: coordinator.address,
			event: COORDINATOR_SIGN_COMPLETED_EVENT,
			fromBlock: "earliest",
			args: {
				sid: request.args.sid,
			},
			strict: true,
		});
		expect(completedRequests.length).toBe(1);
		const completedRequest = completedRequests[0];
		expect(completedRequest.args.sid).toBe(request.args.sid);
		const signature = completedRequest.args.signature;

		// Load group key for verification
		const groupKey = await testClient.readContract({
			...coordinator,
			functionName: "groupKey",
			args: [request.args.gid],
		});
		expect(verifySignature(toPoint(signature.r), signature.z, toPoint(groupKey), proposalMessage)).toBeTruthy();

		// Check that the attestation is correctly tracked
		const attestation = await testClient.readContract({
			...consensus,
			functionName: "getTransactionAttestation",
			args: [proposal.args.epoch, proposal.args.transaction],
		});
		expect(verifySignature(toPoint(attestation.r), attestation.z, toPoint(groupKey), proposalMessage)).toBeTruthy();
	});
});
