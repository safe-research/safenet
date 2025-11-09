import { keccak256 } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, it } from "vitest"; // or '@jest/globals'
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
} from "../frost/types.js";
import { FrostClient } from "./client.js";
import {
	calculateParticipantsRoot,
	hashParticipant,
	verifyMerkleProof,
} from "./merkle.js";
import type { FrostCoordinator, Participant } from "./types.js";

const createRandomAccount = () => privateKeyToAccount(generatePrivateKey());

// --- Tests ---
describe("client", () => {
	it("e2e keygen flow", () => {
		const validatorAddresses = [
			createRandomAccount(),
			createRandomAccount(),
			createRandomAccount(),
		];
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
			): void => {
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
			},
			publishKeygenSecretShares: (
				groupId: GroupId,
				index: bigint,
				verificationShare: FrostPoint,
				peerShares: bigint[],
			): void => {
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
			},
		};
		const clients = validatorAddresses.map(
			(a) => new FrostClient(a.address, coordinator),
		);
		clients.forEach((c) => c.registerParticipants(participants));
		const groupId = keccak256(participantsRoot);
		console.log(
			"------------------------ Trigger Keygen Init ------------------------",
		);
		clients.forEach((c) => {
			console.log(`>>>> Keygen init to ${c.validator()} >>>>`);
			c.handleKeygenInit(groupId, participantsRoot, 3n, 2n);
		});
		console.log(
			"------------------------ Publish Commitments ------------------------",
		);
		clients.forEach((c) =>
			commitmentEvents.forEach((e) => {
				console.log(
					`>>>> Keygen commitment from ${e.index} to ${c.participationIndex(e.groupId)} >>>>`,
				);
				c.handleKeygenCommitment(e.groupId, e.index, e.commits, e.pok);
			}),
		);
		console.log(
			"------------------------ Publish Shares ------------------------",
		);
		clients.forEach((c) =>
			shareEvents.forEach((e) => {
				console.log(
					`>>>> Keygen secrets from ${e.index} to ${c.participationIndex(e.groupId)} >>>>`,
				);
				c.handleKeygenSecrets(e.groupId, e.index, e.peerShares);
			}),
		);
	});
});
