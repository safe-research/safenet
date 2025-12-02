import type { Hex } from "viem";
import { g, neg } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import { groupChallenge } from "./group.js";

export const verifySignature = (
	groupCommitment: FrostPoint,
	combinedSignatureShares: bigint,
	groupPublicKey: FrostPoint,
	msg: Hex,
) => {
	const challenge = groupChallenge(groupCommitment, groupPublicKey, msg);
	const r = g(combinedSignatureShares).add(groupPublicKey.multiply(neg(challenge)));
	if (r.x === 0n && r.y === 0n) return false;
	return r.equals(groupCommitment);
};

export const verifySignatureShare = (
	signatureShare: bigint,
	verificationShare: FrostPoint,
	lagrangeChallenge: bigint,
	groupCommitmentShare: FrostPoint,
) => {
	const sG = g(signatureShare);
	const pki = verificationShare.multiply(lagrangeChallenge);
	const r = groupCommitmentShare.add(pki);
	if (sG.x !== r.x) {
		throw new Error("Invalid signature share");
	}
};
