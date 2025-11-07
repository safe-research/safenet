import type { Address } from "viem";
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
} from "../frost/types.js";
import {
	calculateParticipantsRoot,
	generateParticipantProof,
} from "./merkle.js";
import type { Participant } from "./types.js";

type KeyGenInfo = {
	participantsRoot: string;
	participants: Participant[];
};

type KeyGenMaterial = {
	coefficients: bigint[];
	commitments: FrostPoint[];
};

type KeyGenCommitments = {
	commitments: Map<bigint, FrostPoint[]>;
};

export class FrostClient {
	#validatorAddress: Address = "0x";
	#keyGenInfo = new Map<GroupId, KeyGenInfo>();
	#keyGenMaterial = new Map<GroupId, KeyGenMaterial>();
	#keyGenCommitments = new Map<GroupId, KeyGenCommitments>();

	// Currently we need to add this infor manually, but this should come from the concensus
	prepKeyGen(groupId: GroupId, participants: Participant[]) {
		if (this.#keyGenInfo.has(groupId)) throw Error("Group already known!");
		const participantsRoot = calculateParticipantsRoot(participants);
		this.#keyGenInfo.set(groupId, {
			participantsRoot,
			participants,
		});
	}

	abortKeyGen(groupId: GroupId) {
		if (!this.#keyGenInfo.has(groupId)) return;
		this.#keyGenInfo.delete(groupId);
	}

	handleKeyGenInit(
		groupId: GroupId,
		participantsRoot: string,
		_count: bigint,
		_threshold: bigint,
	) {
		if (this.#keyGenMaterial.has(groupId))
			throw Error("Key generation for this group was already initialized!");
		const info = this.#keyGenInfo.get(groupId);
		if (info === undefined) return;
		if (info.participantsRoot !== participantsRoot)
			throw Error("Unexpected participants root!");
		const participantIndex = info.participants.findIndex(
			(p) => p.address === this.#validatorAddress,
		);
		if (participantIndex < 0)
			throw Error("Cannot determine participant index!");
		const validatorIndex = info.participants.at(participantIndex)?.index;
		if (validatorIndex === undefined)
			throw Error("Cannot determine validator index!");
		// TODO: generate coefficients and commitments
		const coefficients: bigint[] = [];
		const commitments: FrostPoint[] = [];
		// TODO: generate proof of knowledge
		const pok: ProofOfKnowledge = undefined as unknown as ProofOfKnowledge;
		// generate proof of attestation participation
		const poap = generateParticipantProof(info.participants, participantIndex);
		this.publishKeyGenCommitments(
			groupId,
			validatorIndex,
			commitments,
			pok,
			poap,
		);
		this.#keyGenMaterial.set(groupId, {
			coefficients,
			commitments,
		});
	}

	// Round 1.4
	private publishKeyGenCommitments(
		_groupId: GroupId,
		_index: bigint,
		_commits: FrostPoint[],
		_pok: ProofOfKnowledge,
		_poap: ProofOfAttestationParticipation,
	) {
		throw Error("Not implemented");
	}

	handleKeyGenCommitment(
		groupId: GroupId,
		index: number,
		commits: FrostPoint[],
		_pok: ProofOfKnowledge,
	) {
		const info = this.#keyGenInfo.get(groupId);
		if (info === undefined) return;
		const participantIndex = info.participants.findIndex(
			(p) => p.address === this.#validatorAddress,
		);
		if (index === participantIndex) {
			console.info("Do not verify own shares");
			return;
		}
		// TODO: refactor this
		const groupCommits = this.#keyGenCommitments.get(groupId) ?? {
			commitments: new Map(),
		};
		if (groupCommits.commitments.has(index)) {
			throw Error("Commitment for index already known!");
		}
		// verify pok
		groupCommits.commitments.set(index, commits);
		this.#keyGenCommitments.set(groupId, groupCommits);
	}
}
