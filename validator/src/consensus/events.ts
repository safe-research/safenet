import type { Address, PublicClient } from "viem";
import { toPoint } from "../frost/math.js";
import {
	keyGenCommittedEventSchema,
	keyGenEventSchema,
	keyGenSecretSharedEventSchema,
} from "../types/schemas.js";
import type { KeyGenClient } from "./keyGen/client.js";
import { watchKeyGenEvents } from "../service/watchers/keyGen.js";

export const linkClientToCoordinator = (
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
