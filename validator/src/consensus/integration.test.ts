import fs from "node:fs";
import path from "node:path";
import Sqlite3 from "better-sqlite3";
import {
	type Address,
	createPublicClient,
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
import { type Account, type PrivateKeyAccount, privateKeyToAccount } from "viem/accounts";
import { anvil } from "viem/chains";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { silentLogger, testLogger, testMetrics } from "../__tests__/config.js";
import { waitForBlock, waitForBlocks } from "../__tests__/utils.js";
import { hashNonceCommitments, type NonceTree } from "../consensus/signing/nonces.js";
import { toPoint } from "../frost/math.js";
import { calcGenesisGroup, calcGroupContext, calcThreshold } from "../machine/keygen/group.js";
import { createDetector } from "../sentinel/detector.js";
import { SentinelService } from "../sentinel/service.js";
import { createValidatorService, type ValidatorService } from "../service/service.js";
import type { WatcherConfig } from "../shared/watcher.js";
import {
	CONSENSUS_EPOCH_STAGED_EVENT,
	CONSENSUS_ORACLE_TRANSACTION_PROPOSED_EVENT,
	CONSENSUS_TRANSACTION_PROPOSED_EVENT,
	COORDINATOR_EVENTS,
	COORDINATOR_SIGN_COMPLETED_EVENT,
	COORDINATOR_SIGN_EVENT,
	ORACLE_RESULT_EVENT,
} from "../types/abis.js";
import type { ProtocolConfig } from "../types/interfaces.js";
import { participantsForEpoch } from "../utils/participants.js";
import { calcGroupId } from "./keyGen/utils.js";
import { calculateMerkleRoot, calculateParticipantsRoot } from "./merkle.js";
import { verifySignature } from "./signing/verify.js";

const BLOCK_TIME_MS = 250;
const BLOCKS_PER_EPOCH = 30n;
const TEST_RUNTIME_IN_SECONDS = 60;

// Anvil account 6 — sentinel used in the oracle signing flow test
const SENTINEL_PK: Hex = "0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e";
// Anvil account 0 — deployer, MyToken owner, and SentinelOracle arbitrator
const DEPLOYER: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

const ERC20_ABI = parseAbi([
	"function mint(address to, uint256 amount) external",
	"function approve(address spender, uint256 amount) external returns (bool)",
	"function balanceOf(address spender) external view returns (uint256)",
]);

const SENTINEL_ORACLE_ABI = parseAbi([
	"function addSentinel(address sentinel) external",
	"function FEE_TOKEN() external view returns (address)",
	"function REQUEST_FEE() external view returns (uint256)",
	"function VOTING_WINDOW() external view returns (uint256)",
	"function bondMultiplier() external view returns (uint256)",
]);

type BroadcastReturns = Record<string, { value: Address }>;

const loadScriptResults = (script: string): BroadcastReturns | undefined => {
	const file = path.join(process.cwd(), "..", "contracts", "build", "broadcast", script, "31337", "run-latest.json");
	if (!fs.existsSync(file)) return undefined;
	const parsed: { returns: BroadcastReturns } = JSON.parse(fs.readFileSync(file, "utf-8"));
	return parsed.returns;
};

vi.mock(import("../consensus/signing/nonces.js"), async (importOriginal) => {
	const { createNonceTree, ...mod } = await importOriginal();
	return {
		...mod,
		// Creating a nonce tree takes hundreds of milliseconds. Since we are
		// running multiple parallel validators, this means that we may lock up
		// the main thread for seconds at a time, and cause very indeterministic
		// ordering of transaction mining (with transactions being sometimes
		// received by the node several blocks after the action was created, as
		// the NodeJS runtime catches up on running promise continuations),
		// making tests flaky. Mock the nonces tree creation function to create
		// and reuse a single nonce, in order to speed up the method by an order
		// of magnitude. Note that this is **FUNDAMENTALLY UNSAFE** and if used
		// in production will cause the validator to leak its secret signing
		// share. It is, however, OK in order to speed up integration tests.
		createNonceTree: (secret: bigint, size = 1024n): NonceTree => {
			const {
				commitments: [commitment],
			} = createNonceTree(secret, 1n);
			const commitments = [...Array(Number(size))].map(() => commitment);
			const leaves = commitments.map((commitment, i) => hashNonceCommitments(BigInt(i), commitment));
			const root = calculateMerkleRoot(leaves);
			return { commitments, leaves, root };
		},
	};
});

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
	let sentinelService: SentinelService | undefined;

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
		rotateOutEpoch,
		oracleTimeout,
	}: {
		blocksPerEpoch?: bigint;
		timeout?: bigint;
		blockTimeMs?: number;
		rotateOutEpoch?: bigint;
		oracleTimeout?: bigint;
	} = {}) => {
		// Check deployment information is available
		const deployReturns = loadScriptResults("Deploy.s.sol");
		if (!deployReturns) {
			return undefined;
		}

		const erc20Returns = loadScriptResults("DeployERC20.s.sol");
		if (!erc20Returns) {
			return undefined;
		}
		const sentinelOracleReturns = loadScriptResults("DeploySentinelOracle.s.sol");
		if (!sentinelOracleReturns) {
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

		const coordinator = {
			address: deployReturns.coordinator.value,
			abi: parseAbi([
				"function keyGen(bytes32 participants, uint16 count, uint16 threshold, bytes32 context) external returns (bytes32 gid)",
				"function sign(bytes32 gid, bytes32 message) external returns (bytes32 sid)",
				"function groupKey(bytes32 id) external view returns ((uint256 x, uint256 y) key)",
			]),
		} as const;
		testLogger.notice(`Use coordinator at ${coordinator.address}`);
		const consensus = {
			address: deployReturns.consensus.value,
			abi: parseAbi([
				"function proposeTransaction((uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction) external returns (bytes32 safeTxHash)",
				"function proposeOracleTransaction(address oracle, bytes oracleData, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction) external returns (bytes32 safeTxHash)",
				"function getTransactionAttestation(uint64 epoch, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction) external view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
				"function getOracleTransactionAttestationByHash(uint64 epoch, address oracle, bytes32 safeTxHash) external view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
				"function getActiveEpoch() external view returns (uint64 epoch, bytes32 group)",
			]),
		} as const;
		testLogger.notice(`Use consensus at ${consensus.address}`);

		const erc20 = {
			address: erc20Returns.erc20.value,
			abi: ERC20_ABI,
		} as const;
		testLogger.notice(`Use erc20 at ${erc20.address}`);

		const sentinelOracle = {
			address: sentinelOracleReturns.sentinelOracle.value,
			abi: SENTINEL_ORACLE_ABI,
		} as const;
		testLogger.notice(`Use sentinelOracle at ${sentinelOracle.address}`);

		// Private keys from anvil testnet
		const accounts: [PrivateKeyAccount, bigint, bigint | undefined][] = [
			[privateKeyToAccount("0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"), 0n, undefined],
			[privateKeyToAccount("0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"), 0n, undefined],
			[privateKeyToAccount("0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6"), 0n, undefined],
			[privateKeyToAccount("0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a"), 0n, rotateOutEpoch],
			[privateKeyToAccount("0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba"), 2n, undefined],
		];
		const participants = accounts.map(([a, activeFrom, activeBefore]) => {
			return { address: a.address, activeFrom, activeBefore };
		});

		const clients = accounts.map(([a, activeFrom], i) => {
			const logger = i === 0 ? testLogger : silentLogger;
			const config: ProtocolConfig = {
				chainId: 31_337,
				consensus: consensus.address,
				coordinator: coordinator.address,
				staker: a.address,
				participants,
				genesisSalt: zeroHash,
				blocksPerEpoch: blocksPerEpoch ?? BLOCKS_PER_EPOCH,
				keyGenTimeout: timeout,
				signingTimeout: timeout,
				allowedOracles: [sentinelOracle.address],
				oracleTimeout,
			};
			const blockRetryCounts =
				(i & 1) === 0
					? // For half of the validators, use the log querying strategy for unreliable nodes.
						{
							blockAllLogsQueryRetryCount: 2,
							blockSingleQueryRetryCount: 0,
						}
					: {};
			const watcherConfig: WatcherConfig = {
				maxReorgDepth: 1,
				blockTimeOverride: blockTime,
				blockPropagationDelay: Math.floor(blockTime / 5),
				blockRetryDelays: [Math.floor(blockTime / 20), Math.floor(blockTime / 10), Math.floor(blockTime / 5)],
				...blockRetryCounts,
			};
			const service = createValidatorService({
				account: a,
				rpcUrl: "http://127.0.0.1:8545",
				logger,
				config,
				watcherConfig,
				metrics: testMetrics,
				skipGenesis: activeFrom > 0n,
			});
			return {
				account: a,
				service,
			};
		});
		// Store clients for cleanup
		currentClients = clients;

		const genesisGroup = calcGenesisGroup({
			participantsInfo: participants,
			genesisSalt: zeroHash,
		});
		expect(
			await testClient.readContract({
				...consensus,
				functionName: "getActiveEpoch",
			}),
		).toStrictEqual([0n, genesisGroup.id]);

		const triggerKeyGen = async () => {
			if (blockTime > 0) {
				// Disable anvil auto and interval minging
				await testClient.setAutomine(false);
				await testClient.setIntervalMining({ interval: 0 });
				miner = setInterval(async () => {
					await testClient.mine({ blocks: 1 });
				}, blockTime);
			}

			for (const { service } of clients) {
				await service.start();
			}
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
			erc20,
			sentinelOracle,
			triggerKeyGen,
		};
	};

	afterEach(async () => {
		// Cleanup sentinel service before validators to avoid log noise from pending actions
		if (sentinelService !== undefined) {
			try {
				await sentinelService.stop();
			} catch (_e) {}
			sentinelService = undefined;
		}
		// Cleanup services
		for (const { service } of currentClients ?? []) {
			try {
				await service.stop();
			} catch (_e) {}
			currentClients = undefined;
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
		const blocksPerEpoch = 40n;
		const setupInfo = await setup({ timeout: 5n, blocksPerEpoch, rotateOutEpoch: 2n });
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
		await waitForBlock(testClient, blocksPerEpoch);
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
			calculateParticipantsRoot([participants[3].address, participants[1].address, participants[0].address]),
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

		// Restart client and calculate next group
		clients[2].service.start();

		// Wait a few blocks after the epoch, after which the group key will have
		// been calculated.
		await waitForBlock(testClient, blocksPerEpoch + blocksPerEpoch / 2n);

		const expectedGroupEpoch2 = calcGroupId(
			calculateParticipantsRoot([
				participants[1].address,
				participants[0].address,
				participants[2].address,
				participants[4].address,
			]),
			4,
			3,
			calcGroupContext(consensus.address, 2n),
		);
		const expectedKeyEpoch2 = await testClient.readContract({
			...coordinator,
			functionName: "groupKey",
			args: [expectedGroupEpoch2],
		});
		expect(expectedKeyEpoch2.x).not.toBe(0n);
		expect(expectedKeyEpoch2.y).not.toBe(0n);
		expect(stagedGroupKey).not.toStrictEqual(expectedKeyEpoch2);
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
		const epochParticipants = participantsForEpoch(participants, proposedEpoch);
		const expectedGroup = calcGroupId(
			calculateParticipantsRoot(epochParticipants),
			epochParticipants.length,
			calcThreshold(epochParticipants.length),
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
		const setupInfo = await setup({ rotateOutEpoch: 2n });
		if (setupInfo === undefined) {
			skip();
			// Don't run the test code
			return;
		}
		const { coordinator, consensus, triggerKeyGen } = setupInfo;
		const startEpoch = (await testClient.getBlockNumber({ cacheTime: 0 })) / BLOCKS_PER_EPOCH;
		await triggerKeyGen();

		await waitForBlocks(testClient, BLOCKS_PER_EPOCH / 2n);
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
		await waitForBlock(testClient, BLOCKS_PER_EPOCH * 2n);
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
		const safeTxHash = hashTypedData({
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
				safeTxHash,
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
				safeTxHash: proposal.args.safeTxHash,
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

	it("keygen and oracle signing flow", { timeout: TEST_RUNTIME_IN_SECONDS * 1000 * 5 }, async ({ skip }) => {
		const setupInfo = await setup({ oracleTimeout: 30n });
		if (setupInfo === undefined) {
			skip();
			return;
		}
		const { coordinator, consensus, erc20, sentinelOracle, triggerKeyGen } = setupInfo;
		testLogger.notice("Test configuration", {
			erc20: erc20.address,
			sentinelOracle: sentinelOracle.address,
			consensus: consensus.address,
		});

		// Read oracle parameters from deployed contract
		const [feeToken, requestFee, votingWindow, bondMultiplier] = await Promise.all([
			testClient.readContract({
				...sentinelOracle,
				functionName: "FEE_TOKEN",
			}),
			testClient.readContract({
				...sentinelOracle,
				functionName: "REQUEST_FEE",
			}),
			testClient.readContract({
				...sentinelOracle,
				functionName: "VOTING_WINDOW",
			}),
			testClient.readContract({
				...sentinelOracle,
				functionName: "bondMultiplier",
			}),
		]);
		const sentinelBondAmount = requestFee * bondMultiplier;
		testLogger.notice("Oracle configuration", {
			feeToken,
			requestFee,
			votingWindow,
			bondMultiplier,
		});
		const sentinelAccount = privateKeyToAccount(SENTINEL_PK);

		// Impersonate deployer (account 0) — MyToken owner and SentinelOracle arbitrator
		await testClient.impersonateAccount({ address: DEPLOYER });

		await testClient.writeContract({
			...sentinelOracle,
			functionName: "addSentinel",
			args: [sentinelAccount.address],
			account: DEPLOYER,
		});
		// Mint fee token to sentinel (10x bond target for headroom)
		await testClient.writeContract({
			...erc20,
			functionName: "mint",
			args: [sentinelAccount.address, sentinelBondAmount * 10n],
			account: DEPLOYER,
		});
		// Mint fee token to Consensus — msg.sender when it calls postRequest
		await testClient.writeContract({
			...erc20,
			functionName: "mint",
			args: [testClient.account.address, requestFee],
			account: DEPLOYER,
		});
		await testClient.stopImpersonatingAccount({ address: DEPLOYER });

		// Approve SentinelOracle for the request fee
		await testClient.writeContract({
			...erc20,
			functionName: "approve",
			args: [sentinelOracle.address, requestFee],
		});

		// Start the sentinel service — watches Consensus for OracleTransactionProposed
		// and SentinelOracle for NewRequest to commit bonds, finalize, and claim
		const publicClient = createPublicClient({ chain: anvil, transport: http() });
		sentinelService = new SentinelService({
			account: sentinelAccount,
			publicClient,
			config: {
				account: sentinelAccount.address,
				oracle: sentinelOracle.address,
				feeToken: erc20.address,
				consensus: consensus.address,
				chainId: BigInt(anvil.id),
				votingWindow,
			},
			detector: createDetector(),
			logger: testLogger,
			watcherConfig: { maxReorgDepth: 1, blockTimeOverride: BLOCK_TIME_MS },
			database: new Sqlite3(":memory:"),
			metrics: testMetrics,
		});
		await sentinelService.start();

		await triggerKeyGen();
		await waitForBlocks(testClient, BLOCKS_PER_EPOCH / 2n);

		// Propose an oracle-checked transaction — Consensus calls postRequest on SentinelOracle
		testLogger.notice(
			"Oracle fee token balance",
			await testClient.readContract({
				...erc20,
				functionName: "balanceOf",
				args: [consensus.address],
			}),
		);
		const transaction = {
			chainId: 1n,
			safe: "0xb3D9cf8E163bbc840195a97E81F8A34E295B8f39",
			to: "0x74F665BE90ffcd9ce9dcA68cB5875570B711CEca",
			value: 0n,
			data: "0x",
			operation: 0,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		} as const;
		testLogger.notice("Propose oracle transaction", transaction);
		await testClient.writeContractSync({
			...consensus,
			functionName: "proposeOracleTransaction",
			args: [sentinelOracle.address, "0x", transaction],
		});

		// Calculate the proposal message (used as signing target and for attestation lookup)
		const safeTxHash = hashTypedData({
			domain: { chainId: transaction.chainId, verifyingContract: transaction.safe },
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

		// Verify the oracle proposal was emitted by Consensus
		const proposedTransactions = await testClient.getLogs({
			address: consensus.address,
			event: CONSENSUS_ORACLE_TRANSACTION_PROPOSED_EVENT,
			fromBlock: "earliest",
			args: { safeTxHash },
			strict: true,
		});
		expect(proposedTransactions.length).toBe(1);
		const proposal = proposedTransactions[0];
		expect(proposal.args.oracle).toBe(sentinelOracle.address);

		const proposalMessage = hashTypedData({
			domain: { chainId: 31_337, verifyingContract: proposal.address },
			types: {
				OracleTransactionProposal: [
					{ type: "uint64", name: "epoch" },
					{ type: "address", name: "oracle" },
					{ type: "bytes32", name: "safeTxHash" },
				],
			},
			primaryType: "OracleTransactionProposal",
			message: { epoch: proposal.args.epoch, oracle: proposal.args.oracle, safeTxHash: proposal.args.safeTxHash },
		});

		// Wait for the sentinel to commit, wait out the voting window, finalize, and emit OracleResult
		await vi.waitFor(
			async () => {
				const logs = await testClient.getLogs({
					address: sentinelOracle.address,
					event: ORACLE_RESULT_EVENT,
					fromBlock: "earliest",
					strict: true,
				});
				if (logs.length === 0) throw new Error("OracleResult not emitted yet");
			},
			{ timeout: 20_000, interval: 500 },
		);

		const oracleResultLogs = await testClient.getLogs({
			address: sentinelOracle.address,
			event: ORACLE_RESULT_EVENT,
			fromBlock: "earliest",
			strict: true,
		});
		expect(oracleResultLogs).toHaveLength(1);
		expect(oracleResultLogs[0].args.approved).toBe(true);

		// Wait for validators to complete signing of the oracle transaction.
		// COORDINATOR_SIGN_COMPLETED_EVENT doesn't index `message`, so we first
		// wait for COORDINATOR_SIGN_EVENT (which does), then wait for its completion by sid.
		await vi.waitFor(
			async () => {
				const signLogs = await testClient.getLogs({
					address: coordinator.address,
					event: COORDINATOR_SIGN_EVENT,
					fromBlock: "earliest",
					args: { message: proposalMessage },
					strict: true,
				});
				if (signLogs.length === 0) throw new Error("Sign event not emitted yet");
				// biome-ignore lint/style/noNonNullAssertion: length check above guarantees element exists
				const sid = signLogs.at(-1)!.args.sid;
				const completedLogs = await testClient.getLogs({
					address: coordinator.address,
					event: COORDINATOR_SIGN_COMPLETED_EVENT,
					fromBlock: "earliest",
					args: { sid },
					strict: true,
				});
				if (completedLogs.length === 0) throw new Error("SignCompleted not emitted yet");
			},
			{ timeout: 20_000, interval: 500 },
		);

		const signatureRequests = await testClient.getLogs({
			address: coordinator.address,
			event: COORDINATOR_SIGN_EVENT,
			fromBlock: "earliest",
			args: { message: proposalMessage },
			strict: true,
		});
		expect(signatureRequests.length).toBeGreaterThan(0);
		// biome-ignore lint/style/noNonNullAssertion: length check above guarantees element exists
		const request = signatureRequests.at(-1)!;

		const completedRequests = await testClient.getLogs({
			address: coordinator.address,
			event: COORDINATOR_SIGN_COMPLETED_EVENT,
			fromBlock: "earliest",
			args: { sid: request.args.sid },
			strict: true,
		});
		expect(completedRequests.length).toBe(1);
		const signature = completedRequests[0].args.signature;

		const groupKey = await testClient.readContract({
			...coordinator,
			functionName: "groupKey",
			args: [request.args.gid],
		});
		expect(verifySignature(toPoint(signature.r), signature.z, toPoint(groupKey), request.args.message)).toBeTruthy();

		// Verify the oracle transaction attestation is stored in the consensus contract
		const attestation = await testClient.readContract({
			...consensus,
			functionName: "getOracleTransactionAttestationByHash",
			args: [proposal.args.epoch, sentinelOracle.address, proposal.args.safeTxHash],
		});
		expect(
			verifySignature(toPoint(attestation.r), attestation.z, toPoint(groupKey), request.args.message),
		).toBeTruthy();
	});
});
