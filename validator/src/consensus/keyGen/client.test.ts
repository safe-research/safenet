import { type Address, type Hex, keccak256, pad } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, expect, it } from "vitest";
import { createClientStorage, log, testLogger } from "../../__tests__/config.js";
import type { FrostPoint, GroupId, ProofOfKnowledge } from "../../frost/types.js";
import { participantsForEpoch } from "../../utils/participants.js";
import { calculateParticipantsRoot, verifyMerkleProof } from "../merkle.js";
import { KeyGenClient } from "./client.js";
import { calcGroupId } from "./utils.js";

const createRandomAccount = () => privateKeyToAccount(generatePrivateKey());

describe("keyGen", () => {
	it("e2e keygen flow", async () => {
		const count = 3;
		const threshold = 2;
		const participantsInfo = Array.from({ length: Number(count) }, () => ({
			address: createRandomAccount().address,
			activeFrom: 0n,
		}));
		log(`Run test with ${count} validators and threshold ${threshold}`);
		const participants = participantsForEpoch(participantsInfo, 0n);
		const participantsRoot = calculateParticipantsRoot(participants);
		const context = keccak256(participantsRoot);
		const groupId = calcGroupId(participantsRoot, count, threshold, context);
		const commitmentEvents: {
			groupId: GroupId;
			participant: Address;
			encryptionPublicKey: FrostPoint;
			commitments: FrostPoint[];
			pok: ProofOfKnowledge;
		}[] = [];
		const shareEvents: {
			groupId: GroupId;
			participant: Address;
			verificationShare: FrostPoint;
			shares: bigint[];
		}[] = [];
		const clients = participantsInfo.map((p) => {
			const storage = createClientStorage(p.address);
			const client = new KeyGenClient(storage, testLogger);
			return {
				storage,
				client,
			};
		});
		log("------------------------ Trigger Keygen Init and Commitments ------------------------");
		for (const { client } of clients) {
			log(">>>> Keygen and Commit >>>>");
			const { encryptionPublicKey, commitments, poap, pok } = client.setupGroup(participants, threshold, context);
			const participant = client.participant(groupId);
			expect(verifyMerkleProof(participantsRoot, pad(participant).toLowerCase() as Hex, poap)).toBeTruthy();
			log("######################################");
			commitmentEvents.push({
				groupId,
				participant,
				encryptionPublicKey,
				commitments,
				pok,
			});
		}
		log("------------------------ Handle Commitments ------------------------");
		for (const { client } of clients) {
			for (const e of commitmentEvents) {
				log(`>>>> Handle commitment from ${e.participant} >>>>`);
				client.handleKeygenCommitment(e.groupId, e.participant, e.encryptionPublicKey, e.commitments, e.pok);
			}
		}
		log("------------------------ Publish Secret Shares ------------------------");
		for (const { client } of clients) {
			log(`>>>> Publish secret share of ${client.participant(groupId)} >>>>`);
			const { verificationShare, shares } = client.createSecretShares(groupId);
			const participant = client.participant(groupId);
			shareEvents.push({
				groupId,
				participant,
				verificationShare,
				shares,
			});
		}
		log("------------------------ Handle Secret Shares ------------------------");
		for (const { client } of clients) {
			for (const e of shareEvents) {
				log(`>>>> Handle secrets shares from ${e.participant} >>>>`);
				const response = await client.handleKeygenSecrets(e.groupId, e.participant, e.shares);
				expect(response).not.toBe("invalid_share");
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
					participant: storage.participant(groupId),
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
