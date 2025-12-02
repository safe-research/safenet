import { keccak256 } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, expect, it } from "vitest";
import { createStorage, log } from "../../__tests__/config.js";
import type { FrostPoint, GroupId, ProofOfKnowledge } from "../../frost/types.js";
import { calculateParticipantsRoot, hashParticipant, verifyMerkleProof } from "../merkle.js";
import type { Participant } from "../storage/types.js";
import { KeyGenClient } from "./client.js";
import { calcGroupId } from "./utils.js";

const createRandomAccount = () => privateKeyToAccount(generatePrivateKey());

describe("keyGen", () => {
	it("e2e keygen flow", async () => {
		const count = 3n;
		const threshold = count / 2n + 1n;
		const validatorAddresses = Array.from({ length: Number(count) }, () => createRandomAccount());
		log(`Run test with ${count} validators and threshold ${threshold}`);
		const participants: Participant[] = validatorAddresses.map((a, i) => {
			return { id: BigInt(i + 1), address: a.address };
		});
		const participantsRoot = calculateParticipantsRoot(participants);
		const context = keccak256(participantsRoot);
		const groupId = calcGroupId(participantsRoot, count, threshold, context);
		const commitmentEvents: {
			groupId: GroupId;
			participantId: bigint;
			commitments: FrostPoint[];
			pok: ProofOfKnowledge;
		}[] = [];
		const shareEvents: {
			groupId: GroupId;
			id: bigint;
			verificationShare: FrostPoint;
			shares: bigint[];
		}[] = [];
		const clients = validatorAddresses.map((a) => {
			const ids = new Map<GroupId, bigint>();
			const storage = createStorage(a.address);
			const client = new KeyGenClient(storage);
			return {
				ids,
				storage,
				client,
			};
		});
		log("------------------------ Trigger Keygen Init and Commitments ------------------------");
		for (const { client, ids } of clients) {
			log(">>>> Keygen and Commit >>>>");
			const { participantId, commitments, poap, pok } = client.setupGroup(participants, count, threshold, context);
			ids.set(groupId, participantId);
			const leaf = participants.find((p) => p.id === participantId);
			if (leaf === undefined) throw new Error(`Invalid id: ${participantId}`);
			expect(verifyMerkleProof(participantsRoot, hashParticipant(leaf), poap)).toBeTruthy();
			log("######################################");
			commitmentEvents.push({
				groupId,
				participantId,
				commitments,
				pok,
			});
		}
		log("------------------------ Handle Commitments ------------------------");
		for (const { client } of clients) {
			for (const e of commitmentEvents) {
				log(`>>>> Handle commitment from ${e.participantId} by ${client.participantId(e.groupId)} >>>>`);
				client.handleKeygenCommitment(e.groupId, e.participantId, e.commitments, e.pok);
			}
		}
		log("------------------------ Publish Secret Shares ------------------------");
		for (const { client, ids } of clients) {
			log(`>>>> Publish secret share of ${client.participantId(groupId)} >>>>`);
			const { verificationShare, shares } = client.createSecretShares(groupId);
			const id = ids.get(groupId) ?? -1n;
			shareEvents.push({
				groupId,
				id,
				verificationShare,
				shares,
			});
		}
		log("------------------------ Handle Secret Shares ------------------------");
		for (const { client } of clients) {
			for (const e of shareEvents) {
				log(`>>>> Handle secrets shares from ${e.id} by ${client.participantId(e.groupId)} >>>>`);
				await client.handleKeygenSecrets(e.groupId, e.id, e.shares);
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
