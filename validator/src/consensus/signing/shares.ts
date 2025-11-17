import { addmod, mulmod } from "../../frost/math.js";
import type { SecretNonceCommitments } from "./nonces.js";

export const lagrangeChallenge = (
	lagrangeCoefficient: bigint,
	challenge: bigint,
): bigint => mulmod(challenge, lagrangeCoefficient);

export const createSignatureShare = (
	privateKeyShare: bigint,
	nonces: SecretNonceCommitments,
	bindingFactor: bigint,
	lagrangeChallenge: bigint,
): bigint => {
	return addmod(
		nonces.hidingNonce,
		addmod(
			mulmod(nonces.bindingNonce, bindingFactor),
			mulmod(lagrangeChallenge, privateKeyShare),
		),
	);
};
