import { type Address, type Hex, keccak256, pad } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, expect, it } from "vitest";
import { log, testLogger } from "../../__tests__/config.js";
import { createClientStorage } from "../../__tests__/utils.js";
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
			const storage = createClientStorage();
			const client = new KeyGenClient(storage, testLogger);
			return {
				account: p.address,
				storage,
				client,
			};
		});
		log("------------------------ Trigger Keygen Init and Commitments ------------------------");
		for (const { client, account } of clients) {
			log(">>>> Keygen and Commit >>>>");
			const { groupId } = client.setupGroup(participants, threshold, context);
			const { encryptionPublicKey, commitments, poap, pok } = client.setupKeyGen(
				groupId,
				account,
				participants,
				threshold,
			);
			expect(verifyMerkleProof(participantsRoot, pad(account).toLowerCase() as Hex, poap)).toBeTruthy();
			log("######################################");
			commitmentEvents.push({
				groupId,
				participant: account,
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
		for (const { client, account } of clients) {
			log(`>>>> Publish secret share of ${account} >>>>`);
			const { verificationShare, shares } = client.createSecretShares(groupId, account);
			shareEvents.push({
				groupId,
				participant: account,
				verificationShare,
				shares,
			});
		}
		log("------------------------ Handle Secret Shares ------------------------");
		for (const { client, account } of clients) {
			for (const e of shareEvents) {
				log(`>>>> Handle secrets shares from ${e.participant} >>>>`);
				const response = await client.handleKeygenSecrets(e.groupId, account, e.participant, e.shares);
				expect(response).not.toBe("invalid_share");
			}
		}
		for (const { storage, account } of clients) {
			for (const groupId of storage.knownGroups()) {
				const publicKey = storage.publicKey(groupId);
				const verificationShare = storage.verificationShare(groupId, account);
				log({
					groupId,
					signingShare: storage.signingShare(groupId, account),
					participants: storage.participants(groupId),
					participant: account,
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
