import type { Address, PublicClient } from "viem";
import { toPoint } from "../frost/math.js";
import { watchKeyGenEvents } from "../service/watchers/keyGen.js";
import { watchSignEvents } from "../service/watchers/signing.js";
import {
	epochProposedEventSchema,
	keyGenCommittedEventSchema,
	keyGenEventSchema,
	keyGenSecretSharedEventSchema,
	nonceCommitmentsEventSchema,
	nonceCommitmentsHashEventSchema,
	signRequestEventSchema,
} from "../types/schemas.js";
import type { KeyGenClient } from "./keyGen/client.js";
import type { SigningClient } from "./signing/client.js";
import { watchConsensusEvents } from "../service/watchers/consensus.js";
import { VerificationEngine } from "./verify/engine.js";
import { EpochRolloverPacket } from "./verify/rollover/schemas.js";

export const linkKeyGenClientToCoordinator = (
	frostClient: KeyGenClient,
	publicClient: PublicClient,
	coordinatorAddress: Address,
) => {
	watchKeyGenEvents({
		client: publicClient,
		target: coordinatorAddress,
		onKeyGenInit: async (e) => {
			const event = keyGenEventSchema.parse(e);
			return frostClient.handleKeygenInit(
				event.gid,
				event.participants,
				event.count,
				event.threshold,
			);
		},
		onKeyGenCommitment: async (e) => {
			const event = keyGenCommittedEventSchema.parse(e);
			return frostClient.handleKeygenCommitment(
				event.gid,
				event.identifier,
				event.commitment.c.map((c) => toPoint(c)),
				{
					r: toPoint(event.commitment.r),
					mu: event.commitment.mu,
				},
			);
		},
		onKeyGenSecrets: async (e) => {
			const event = keyGenSecretSharedEventSchema.parse(e);
			return frostClient.handleKeygenSecrets(
				event.gid,
				event.identifier,
				event.share.f,
			);
		},
		onError: console.error,
	});
};

export const linkSigningClientToCoordinator = (
	frostClient: SigningClient,
	publicClient: PublicClient,
	coordinatorAddress: Address,
) => {
	watchSignEvents({
		client: publicClient,
		target: coordinatorAddress,
		onNewNonceCommitmentsHash: async (e) => {
			const event = nonceCommitmentsHashEventSchema.parse(e);
			return frostClient.handleNonceCommitmentsHash(
				event.gid,
				event.identifier,
				event.commitment,
				event.chunk,
			);
		},
		onNonceCommitmentsRevealed: async (e) => {
			const event = nonceCommitmentsEventSchema.parse(e);
			return frostClient.handleNonceCommitments(event.sid, event.identifier, {
				hidingNonceCommitment: toPoint(event.nonces.d),
				bindingNonceCommitment: toPoint(event.nonces.e),
			});
		},
		onSignRequest: async (e) => {
			const event = signRequestEventSchema.parse(e);
			return frostClient.handleSignatureRequest(
				event.gid,
				event.sid,
				event.message,
				event.sequence,
			);
		},
		onError: console.error,
	});
};

export const linkSigningClientToConsensus = (
	verificationEngine: VerificationEngine,
	publicClient: PublicClient,
	consensusAddress: Address,
) => {
	watchConsensusEvents({
		client: publicClient,
		target: consensusAddress,
		onEpochProposed: async (e) => {
			const event = epochProposedEventSchema.parse(e);
			const packet: EpochRolloverPacket = {
				type: "epoch_rollover_packet",
				domain: {
					chain: BigInt(publicClient.chain?.id ?? 0),
					consensus: consensusAddress
				},
				rollover: {
					activeEpoch: event.activeEpoch,
					proposedEpoch: event.proposedEpoch,
					rolloverAt: event.timestamp,
					groupKeyX: event.groupKey.x,
					groupKeyY: event.groupKey.y
				}
			}
			await verificationEngine.verify(packet)
		},
		onError: console.error,
	})
}
