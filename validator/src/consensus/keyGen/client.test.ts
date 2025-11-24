import { type Address, type Hex, keccak256, zeroAddress } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, expect, it } from "vitest";
import { log } from "../../__tests__/logging.js";
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
import type { KeyGenCoordinator, Participant } from "../types.js";
import { KeyGenClient } from "./client.js";
import { calcGroupId } from "./utils.js";

const createRandomAccount = () => privateKeyToAccount(generatePrivateKey());

describe("keyGen", () => {
	it("e2e keygen flow", async () => {
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
		const context = keccak256(participantsRoot);
		const commitmentEvents: {
			groupId: GroupId;
			id: bigint;
			commits: FrostPoint[];
			pok: ProofOfKnowledge;
		}[] = [];
		const shareEvents: {
			groupId: GroupId;
			id: bigint;
			verificationShare: FrostPoint;
			peerShares: bigint[];
		}[] = [];
		const clients = validatorAddresses.map((a) => {
			const participantIdMapping = new Map<GroupId, bigint>();
			const coordinator: KeyGenCoordinator = {
				triggerKeygenAndCommit: (
					root: Hex,
					c: bigint,
					t: bigint,
					ctx: Hex,
					id: bigint,
					commits: FrostPoint[],
					pok: ProofOfKnowledge,
					poap: ProofOfAttestationParticipation,
				): Promise<Hex> => {
					const groupId = calcGroupId(root, c, t, ctx);
					participantIdMapping.set(groupId, id);
					expect(root).toBe(participantsRoot);
					expect(c).toBe(count);
					expect(t).toBe(threshold);
					expect(ctx).toBe(context);
					log("##### Received KeygenCommitments #####");
					log({
						groupId,
						id,
						commits,
						pok,
						poap,
					});
					const leaf = participants.find((p) => p.id === id);
					if (leaf === undefined) throw Error(`Invliad id: ${id}`);
					expect(
						verifyMerkleProof(participantsRoot, hashParticipant(leaf), poap),
					).toBeTruthy();
					log("######################################");
					commitmentEvents.push({
						groupId,
						id,
						commits,
						pok,
					});
					return Promise.resolve("0x");
				},
				publishKeygenCommitments: (
					_groupId: GroupId,
					_id: bigint,
					_commits: FrostPoint[],
					_pok: ProofOfKnowledge,
					_poap: ProofOfAttestationParticipation,
				): Promise<Hex> => {
					return Promise.reject("Should not be called");
				},
				publishKeygenSecretShares: (
					groupId: GroupId,
					verificationShare: FrostPoint,
					peerShares: bigint[],
					callbackContext: Hex,
				): Promise<Hex> => {
					log("##### Received KeygenSecretShares #####");
					log({
						groupId,
						verificationShare,
						peerShares,
					});
					log("#######################################");
					expect(callbackContext).toBe("0x5afe5afe");
					const id = participantIdMapping.get(groupId) ?? -1n;
					shareEvents.push({
						groupId,
						id,
						verificationShare,
						peerShares,
					});
					return Promise.resolve("0x");
				},
				chainId: (): bigint => 0n,
				coordinator: (): Address => zeroAddress,
			};
			const storage = new InMemoryStorage(a.address);
			const client = new KeyGenClient(storage, coordinator);
			client.registerParticipants(participants);
			return {
				storage,
				client,
			};
		});
		log(
			"------------------------ Trigger Keygen Init ------------------------",
		);
		for (const { client } of clients) {
			log(`>>>> Keygen and Commit >>>>`);
			await client.triggerKeygenAndCommit(
				participantsRoot,
				count,
				threshold,
				context,
			);
		}
		log(
			"------------------------ Publish Commitments ------------------------",
		);
		for (const { client } of clients) {
			for (const e of commitmentEvents) {
				log(
					`>>>> Keygen commitment from ${e.id} to ${client.participantId(e.groupId)} >>>>`,
				);
				await client.handleKeygenCommitment(
					e.groupId,
					e.id,
					e.commits,
					e.pok,
					"0x5afe5afe",
				);
			}
		}
		log("------------------------ Publish Shares ------------------------");
		for (const { client } of clients) {
			for (const e of shareEvents) {
				log(
					`>>>> Keygen secrets from ${e.id} to ${client.participantId(e.groupId)} >>>>`,
				);
				await client.handleKeygenSecrets(e.groupId, e.id, e.peerShares);
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
