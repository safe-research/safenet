import { keccak_256 } from "@noble/hashes/sha3.js";
import type { Hex } from "viem";
import { hexToBytes } from "viem";
import type { FrostPoint } from "./math.js";
import { add, g, multiply } from "./math.js";

export const verifySignature = (
	groupCommitment: FrostPoint,
	combinedSignatureShares: bigint,
	groupPublicKey: FrostPoint,
	msg: Hex,
): boolean => {
	const challenge = groupChallenge(groupCommitment, groupPublicKey, msg);
	const r = add(g(combinedSignatureShares), multiply(groupPublicKey, -challenge));
	if (r.x === 0n && r.y === 0n) return false;
	return r.equals(groupCommitment);
};

export const groupChallenge = (r: FrostPoint, groupPublicKey: FrostPoint, msg: Hex): bigint => {
	// Hash r.x, r.y, groupPublicKey.x, groupPublicKey.y, and msg
	const msgBytes = hexToBytes(msg);
	const msgHash = keccak_256(msgBytes);
	const data = new Uint8Array(32 + 32 + 32 + 32 + 32); // r.x + r.y + groupPublicKey.x + groupPublicKey.y + msgHash
	data.set(serialize(r.x), 0);
	data.set(serialize(r.y), 32);
	data.set(serialize(groupPublicKey.x), 64);
	data.set(serialize(groupPublicKey.y), 96);
	data.set(msgHash, 128);

	return BigInt(`0x${Buffer.from(keccak_256(data)).toString("hex")}`);
};

const serialize = (val: bigint): Uint8Array => {
	const hex = val.toString(16).padStart(64, "0");
	return new Uint8Array(Buffer.from(hex, "hex"));
};
