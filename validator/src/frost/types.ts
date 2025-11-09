import type { Hex } from "viem";

export type FrostPoint = {
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

export type GroupId = Hex;
