import { randomBytes } from "node:crypto";
import { bytesToNumberBE } from "@noble/curves/utils.js";
import {
	createPublicClient,
	createWalletClient,
	type Hex,
	http,
	keccak256,
	parseAbi,
} from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { anvil } from "viem/chains";
import { describe, it } from "vitest"; // or '@jest/globals'
import { toPoint } from "../frost/math.js";
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
} from "../frost/types.js";
import { watchCoordinatorEvents } from "../service/watchers.js";
import {
	keyGenCommittedEventSchema,
	keyGenEventSchema,
	keyGenSecretSharedEventSchema,
} from "../types/schemas.js";
import { FrostClient } from "./client.js";
import { OnchainCoordinator } from "./coordinator.js";
import {
	calculateParticipantsRoot,
	hashParticipant,
	verifyMerkleProof,
} from "./merkle.js";
import type { FrostCoordinator, Participant } from "./types.js";

const createRandomAccount = () => privateKeyToAccount(generatePrivateKey());

// --- Tests ---
describe("client", () => {
	it("e2e keygen flow", async () => {
		const count = 3n;
		const threshold = count / 2n + 1n;
		const validatorAddresses = Array.from({ length: Number(count) }, () =>
			createRandomAccount(),
		);
		console.log(`Run test with ${count} validators and threshold ${threshold}`);
		const participants: Participant[] = validatorAddresses.map((a, i) => {
			return { index: BigInt(i + 1), address: a.address };
		});
		const participantsRoot = calculateParticipantsRoot(participants);
		const commitmentEvents: {
			groupId: GroupId;
			index: bigint;
			commits: FrostPoint[];
			pok: ProofOfKnowledge;
		}[] = [];
		const shareEvents: {
			groupId: GroupId;
			index: bigint;
			verificationShare: FrostPoint;
			peerShares: bigint[];
		}[] = [];
		const coordinator: FrostCoordinator = {
			publishKeygenCommitments: (
				groupId: GroupId,
				index: bigint,
				commits: FrostPoint[],
				pok: ProofOfKnowledge,
				poap: ProofOfAttestationParticipation,
			): Promise<Hex> => {
				console.log("##### Received KeygenCommitments #####");
				console.log({
					groupId,
					index,
					commits,
					pok,
					poap,
				});
				const leaf = participants.find((p) => p.index === index);
				if (leaf === undefined) throw Error(`Invliad index: ${index}`);
				console.log({
					validMerkleProof: verifyMerkleProof(
						participantsRoot,
						hashParticipant(leaf),
						poap,
					),
				});
				console.log("######################################");
				commitmentEvents.push({
					groupId,
					index,
					commits,
					pok,
				});
				return Promise.resolve("0x");
			},
			publishKeygenSecretShares: (
				groupId: GroupId,
				index: bigint,
				verificationShare: FrostPoint,
				peerShares: bigint[],
			): Promise<Hex> => {
				console.log("##### Received KeygenSecretShares #####");
				console.log({
					groupId,
					index,
					verificationShare,
					peerShares,
				});
				console.log("#######################################");
				shareEvents.push({
					groupId,
					index,
					verificationShare,
					peerShares,
				});
				return Promise.resolve("0x");
			},
		};
		const clients = validatorAddresses.map(
			(a) => new FrostClient(a.address, coordinator),
		);
		for (const c of clients) c.registerParticipants(participants);
		const groupId = keccak256(participantsRoot);
		console.log(
			"------------------------ Trigger Keygen Init ------------------------",
		);
		for (const c of clients) {
			console.log(`>>>> Keygen init to ${c.validator()} >>>>`);
			await c.handleKeygenInit(groupId, participantsRoot, count, threshold);
		}
		console.log(
			"------------------------ Publish Commitments ------------------------",
		);
		for (const c of clients) {
			for (const e of commitmentEvents) {
				console.log(
					`>>>> Keygen commitment from ${e.index} to ${c.participationIndex(e.groupId)} >>>>`,
				);
				c.handleKeygenCommitment(e.groupId, e.index, e.commits, e.pok);
			}
		}
		console.log(
			"------------------------ Publish Shares ------------------------",
		);
		for (const c of clients) {
			for (const e of shareEvents) {
				console.log(
					`>>>> Keygen secrets from ${e.index} to ${c.participationIndex(e.groupId)} >>>>`,
				);
				c.handleKeygenSecrets(e.groupId, e.index, e.peerShares);
			}
		}
	});

	it.skip("integration keygen flow", { timeout: 30000 }, async () => {
		// Make sure to first start the Anvil testnode (run `anvil` in the root)
		// and run the deployment script: forge script DeployScript --rpc-url http://127.0.0.1:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 --broadcast
		// Private key from Anvil testnet
		const accounts = [
			privateKeyToAccount(
				"0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
			),
			privateKeyToAccount(
				"0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
			),
			privateKeyToAccount(
				"0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6",
			),
		];
		const participants: Participant[] = accounts.map((a, i) => {
			return { index: BigInt(i + 1), address: a.address };
		});
		const coordinatorAddress = "0x601CDd9C9e743AEb40106865775FDB073ee4E889";
		accounts.map((a) => {
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
			const coordinator = new OnchainCoordinator(
				publicClient,
				signingClient,
				coordinatorAddress,
			);
			const client = new FrostClient(a.address, coordinator);
			watchCoordinatorEvents({
				client: publicClient,
				target: coordinatorAddress,
				onKeyGenInit: async (e) => {
					const event = keyGenEventSchema.parse(e);
					return client.handleKeygenInit(
						event.id,
						event.participants,
						event.count,
						event.threshold,
					);
				},
				onKeyGenCommitment: async (e) => {
					const event = keyGenCommittedEventSchema.parse(e);
					return client.handleKeygenCommitment(
						event.id,
						event.index,
						event.commitment.c.map((c) => toPoint(c)),
						{
							r: toPoint(event.commitment.r),
							mu: event.commitment.mu,
						},
					);
				},
				onKeyGenSecrets: async (e) => {
					const event = keyGenSecretSharedEventSchema.parse(e);
					return client.handleKeygenSecrets(
						event.id,
						event.index,
						event.share.f,
					);
				},
				onError: console.error,
			});
			client.registerParticipants(participants);
			return client;
		});
		const participantsRoot = calculateParticipantsRoot(participants);
		const initiatorClient = createWalletClient({
			chain: anvil,
			transport: http(),
			account: accounts[0],
		});
		const abi = parseAbi([
			"function keygen(uint96 nonce, bytes32 participants, uint128 count, uint128 threshold) external",
		]);
		await initiatorClient.writeContract({
			address: coordinatorAddress,
			abi: abi,
			functionName: "keygen",
			args: [
				bytesToNumberBE(randomBytes(12)),
				participantsRoot,
				BigInt(accounts.length),
				BigInt(Math.ceil(accounts.length / 2)),
			],
		});
		await new Promise((resolve) => setTimeout(resolve, 30000));
	});
});
