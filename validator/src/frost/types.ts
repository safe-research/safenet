import type { Hex } from "viem";

// TODO: remove, this is for dev purposes
export type FrostMath = {
	ecdh(
		msg: bigint,
		senderPrivateKey: bigint,
		receiverPublicKey: FrostPoint,
	): bigint;

	createCoefficients(threshold: bigint): bigint[];

	createProofOfKnowledge(
		groupId: GroupId,
		index: bigint,
		coefficients: bigint[],
	): ProofOfKnowledge;

	createCommitments(coefficients: bigint[]): FrostPoint[];

	verifyCommitments(
		groupId: GroupId,
		index: bigint,
		commitments: FrostPoint[],
		proof: ProofOfKnowledge,
	): void;

	createVerificationShare(
		allCommitments: Map<bigint, FrostPoint[]>,
		senderIndex: bigint,
	): FrostPoint;

	createSigningShare(secretShares: Map<bigint, bigint>): bigint;

	evalCommitment(commitments: FrostPoint[], x: bigint): FrostPoint;

	evalPoly(coefficient: bigint[], x: bigint): bigint;

	verifyKey(publicKey: FrostPoint, privateKey: bigint): void;
};
// TODO: remove, this is for dev purposes
export const FROST_MATH: FrostMath = {} as unknown as FrostMath;

export type FrostPoint = {
	readonly px: bigint;
	readonly py: bigint;
	readonly pz: bigint;
	get x(): bigint;
	get y(): bigint;
	assertValidity(): void;
	double(): FrostPoint;
	negate(): FrostPoint;
	add(other: FrostPoint): FrostPoint;
	subtract(other: FrostPoint): FrostPoint;
	equals(other: FrostPoint): boolean;
	multiply(scalar: bigint): FrostPoint;
};

export type ProofOfKnowledge = {
	r: FrostPoint;
	mu: bigint;
};

export type ProofOfAttestationParticipation = Hex[];

export type GroupId = bigint;
