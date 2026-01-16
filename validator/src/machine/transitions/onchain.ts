import type { ALL_EVENTS } from "../../types/abis.js";
import type { Log } from "../../watcher/events.js";
import {
	epochProposedEventSchema,
	epochStagedEventSchema,
	keyGenCommittedEventSchema,
	keyGenComplaintRespondedEventSchema,
	keyGenComplaintSubmittedEventSchema,
	keyGenConfirmedEventSchema,
	keyGenEventSchema,
	keyGenSecretSharedEventSchema,
	nonceCommitmentsEventSchema,
	nonceCommitmentsHashEventSchema,
	signatureShareEventSchema,
	signedEventSchema,
	signRequestEventSchema,
	transactionAttestedEventSchema,
	transactionProposedEventSchema,
} from "./schemas.js";
import type { EventTransition } from "./types.js";

export type ProtocolLog = Pick<Log<typeof ALL_EVENTS>, "blockNumber" | "logIndex" | "eventName" | "args">;

export const logToTransition = ({
	blockNumber: block,
	logIndex: index,
	eventName,
	args: eventArgs,
}: ProtocolLog): EventTransition => {
	switch (eventName) {
		case "KeyGenCommitted": {
			const args = keyGenCommittedEventSchema.parse(eventArgs);
			return {
				id: "event_key_gen_committed",
				block,
				index,
				...args,
			};
		}
		case "KeyGenSecretShared": {
			const args = keyGenSecretSharedEventSchema.parse(eventArgs);
			return {
				id: "event_key_gen_secret_shared",
				block,
				index,
				...args,
			};
		}
		case "KeyGenComplained": {
			const args = keyGenComplaintSubmittedEventSchema.parse(eventArgs);
			return {
				id: "event_key_gen_complaint_submitted",
				block,
				index,
				...args,
			};
		}
		case "KeyGenComplaintResponded": {
			const args = keyGenComplaintRespondedEventSchema.parse(eventArgs);
			return {
				id: "event_key_gen_complaint_responded",
				block,
				index,
				...args,
			};
		}
		case "KeyGenConfirmed": {
			const args = keyGenConfirmedEventSchema.parse(eventArgs);
			return {
				id: "event_key_gen_confirmed",
				block,
				index,
				...args,
			};
		}
		case "Preprocess": {
			const args = nonceCommitmentsHashEventSchema.parse(eventArgs);
			return {
				id: "event_nonce_commitments_hash",
				block,
				index,
				...args,
			};
		}
		case "Sign": {
			const args = signRequestEventSchema.parse(eventArgs);
			return {
				id: "event_sign_request",
				block,
				index,
				...args,
			};
		}
		case "SignRevealedNonces": {
			const args = nonceCommitmentsEventSchema.parse(eventArgs);
			return {
				id: "event_nonce_commitments",
				block,
				index,
				...args,
			};
		}
		case "SignShared": {
			const args = signatureShareEventSchema.parse(eventArgs);
			return {
				id: "event_signature_share",
				block,
				index,
				...args,
			};
		}
		case "SignCompleted": {
			const args = signedEventSchema.parse(eventArgs);
			return {
				id: "event_signed",
				block,
				index,
				...args,
			};
		}
		case "EpochStaged": {
			const args = epochStagedEventSchema.parse(eventArgs);
			return {
				id: "event_epoch_staged",
				block,
				index,
				...args,
			};
		}
		case "TransactionProposed": {
			const args = transactionProposedEventSchema.parse(eventArgs);
			return {
				id: "event_transaction_proposed",
				block,
				index,
				...args,
			};
		}
		case "TransactionAttested": {
			const args = transactionAttestedEventSchema.parse(eventArgs);
			return {
				id: "event_transaction_attested",
				block,
				index,
				...args,
			};
		}
		case "KeyGen": {
			const args = keyGenEventSchema.parse(eventArgs);
			return {
				id: "event_key_gen",
				block,
				index,
				...args,
			};
		}
		case "EpochProposed": {
			const args = epochProposedEventSchema.parse(eventArgs);
			return {
				id: "event_epoch_proposed",
				block,
				index,
				...args,
			};
		}
	}
};
