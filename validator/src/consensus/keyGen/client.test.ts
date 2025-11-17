import { type Hex, keccak256 } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, it } from "vitest";
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
} from "../../frost/types.js";
import {
	calculateParticipantsRoot,
	hashParticipant,
	verifyMerkleProof,
} from "../merkle.js";
import { InMemoryStorage } from "../storage.js";
import type { FrostCoordinator, Participant } from "../types.js";
import { KeyGenClient } from "./client.js";

const createRandomAccount = () => privateKeyToAccount(generatePrivateKey());

// --- Tests ---
describe("keyGen", () => {
	it("e2e keygen flow", async () => {
		const log = (msg: unknown) => {
			if (process.env.VERBOSE) console.log(msg);
		};
		const count = 3n;
		const threshold = count / 2n + 1n;
		const validatorAddresses = Array.from({ length: Number(count) }, () =>
			createRandomAccount(),
		);
		log(`Run test with ${count} validators and threshold ${threshold}`);
		const participants: Participant[] = validatorAddresses.map((a, i) => {
			return { id: BigInt(i + 1), address: a.address };
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
		const clients = validatorAddresses.map((a) => {
			const participantIndexMapping = new Map<GroupId, bigint>();
			const coordinator: FrostCoordinator = {
				publishKeygenCommitments: (
					groupId: GroupId,
					index: bigint,
					commits: FrostPoint[],
					pok: ProofOfKnowledge,
					poap: ProofOfAttestationParticipation,
				): Promise<Hex> => {
					participantIndexMapping.set(groupId, index);
					log("##### Received KeygenCommitments #####");
					log({
						groupId,
						index,
						commits,
						pok,
						poap,
					});
					const leaf = participants.find((p) => p.id === index);
					if (leaf === undefined) throw Error(`Invliad index: ${index}`);
					log({
						validMerkleProof: verifyMerkleProof(
							participantsRoot,
							hashParticipant(leaf),
							poap,
						),
					});
					log("######################################");
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
					verificationShare: FrostPoint,
					peerShares: bigint[],
				): Promise<Hex> => {
					log("##### Received KeygenSecretShares #####");
					log({
						groupId,
						verificationShare,
						peerShares,
					});
					log("#######################################");
					const index = participantIndexMapping.get(groupId) ?? -1n;
					shareEvents.push({
						groupId,
						index,
						verificationShare,
						peerShares,
					});
					return Promise.resolve("0x");
				},
			};
			const storage = new InMemoryStorage(a.address);
			const client = new KeyGenClient(storage, coordinator);
			client.registerParticipants(participants);
			return {
				storage,
				client,
			};
		});
		const groupId = keccak256(participantsRoot);
		log(
			"------------------------ Trigger Keygen Init ------------------------",
		);
		for (const { client } of clients) {
			log(`>>>> Keygen init >>>>`);
			await client.handleKeygenInit(
				groupId,
				participantsRoot,
				count,
				threshold,
			);
		}
		log(
			"------------------------ Publish Commitments ------------------------",
		);
		for (const { client } of clients) {
			for (const e of commitmentEvents) {
				log(
					`>>>> Keygen commitment from ${e.index} to ${client.participationId(e.groupId)} >>>>`,
				);
				await client.handleKeygenCommitment(
					e.groupId,
					e.index,
					e.commits,
					e.pok,
				);
			}
		}
		log("------------------------ Publish Shares ------------------------");
		for (const { client } of clients) {
			for (const e of shareEvents) {
				log(
					`>>>> Keygen secrets from ${e.index} to ${client.participationId(e.groupId)} >>>>`,
				);
				await client.handleKeygenSecrets(e.groupId, e.index, e.peerShares);
			}
		}
		for (const { storage } of clients) {
			log(storage.accountAddress());
			for (const groupId of storage.knownGroups()) {
				const publicKey = storage.publicKey(groupId);
				const verificationShare = storage.verificationShare(groupId);
				log({
					groupId,
					signingShare: storage.signingShare(groupId),
					participants: storage.participants(groupId),
					participantId: storage.participantId(groupId),
					verificationShare: {
						x: verificationShare?.x,
						y: verificationShare?.y,
					},
					publicKey: {
						x: publicKey?.x,
						y: publicKey?.y,
					},
				});
			}
		}
	});
});
