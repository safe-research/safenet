import { concatBytes } from "@noble/hashes/utils.js";
import type { Hex } from "viem";
import { hexToBytes } from "viem";
import { h2 } from "@/lib/frost/hashes.js";
import type { FrostPoint } from "./math.js";
import { g, neg } from "./math.js";

export const verifySignature = (
	groupCommitment: FrostPoint,
	combinedSignatureShares: bigint,
	groupPublicKey: FrostPoint,
	msg: Hex,
): boolean => {
	const challenge = groupChallenge(groupCommitment, groupPublicKey, msg);
	const r = g(combinedSignatureShares).add(groupPublicKey.multiply(neg(challenge)));
	if (r.x === 0n && r.y === 0n) return false;
	return r.equals(groupCommitment);
};

const groupChallenge = (groupCommitment: FrostPoint, groupPublicKey: FrostPoint, message: Hex): bigint => {
	return h2(concatBytes(groupCommitment.toBytes(true), groupPublicKey.toBytes(true), hexToBytes(message)));
};
