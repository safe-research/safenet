import type { FrostPoint } from "./types.js";

export const ecdh = (msg: bigint, senderPrivateKey: bigint, receiverPublicKey: FrostPoint): bigint => {
	return msg ^ receiverPublicKey.multiply(senderPrivateKey).x;
};
