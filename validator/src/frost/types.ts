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
	toBytes(isCompressed?: boolean): Uint8Array;
	toHex(isCompressed?: boolean): string;
};

export type ProofOfKnowledge = {
	r: FrostPoint;
	mu: bigint;
};

export type EncryptionKey = {
	secretKey: bigint;
	publicKey: FrostPoint;
};

export type ProofOfAttestationParticipation = Hex[];

export type GroupId = Hex;

export type SignatureId = Hex;

export type FrostSignature = {
	z: bigint;
	r: FrostPoint;
};
