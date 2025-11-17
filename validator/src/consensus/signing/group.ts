import { concatBytes } from "@noble/hashes/utils.js";
import { type Hex, hexToBytes } from "viem";
import { h2 } from "../../frost/hashes.js";
import { divmod, mulmod, submod } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";

export const groupChallenge = (
	groupCommitment: FrostPoint,
	groupPublicKey: FrostPoint,
	message: Hex,
): bigint => {
	return h2(
		concatBytes(
			groupCommitment.toBytes(true),
			groupPublicKey.toBytes(true),
			hexToBytes(message),
		),
	);
};

export const lagrangeCoefficient = (signers: bigint[], id: bigint): bigint => {
	if (signers.indexOf(id) < 0) throw Error(`${id} not part of signers`);
	let numerator = 1n;
	let denominator = 1n;

	for (const signer of signers) {
		// Skip the participant index.
		if (signer === id) continue;
		numerator = mulmod(numerator, signer);
		denominator = mulmod(denominator, submod(signer, id));
	}

	// Return the lagrange coefficient.
	return divmod(numerator, denominator);
};
