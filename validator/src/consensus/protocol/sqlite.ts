import type { Database } from "better-sqlite3";
import type { Hex } from "viem";
import { z } from "zod";
import { toPoint } from "../../frost/math.js";
import type { GroupId, ParticipantId, SignatureId } from "../../frost/types.js";
import { checkedAddressSchema, hexBytes32Schema, hexDataSchema } from "../../types/schemas.js";
import { jsonReplacer } from "../../utils/json.js";
import { SqliteQueue } from "../../utils/queue.js";
import type { EthTransactionData, TransactionStorage } from "./onchain.js";
import type { ActionWithTimeout } from "./types.js";

const groupIdSchema = hexBytes32Schema.transform((v) => v as GroupId);
const coercedBigIntSchema = z.coerce.bigint().nonnegative();
const participantIdSchema = coercedBigIntSchema.transform((v) => v as ParticipantId);
const signatureIdSchema = hexBytes32Schema.transform((v) => v as SignatureId);

// Complex objects from imports
const frostPointSchema = z
	.object({
		x: coercedBigIntSchema,
		y: coercedBigIntSchema,
	})
	.transform((p) => toPoint(p));

const proofOfKnowledgeSchema = z.object({
	r: frostPointSchema,
	mu: coercedBigIntSchema,
});

const proofOfAttestationParticipationSchema = z.array(hexBytes32Schema);

const publicNonceCommitmentsSchema = z.object({
	hidingNonceCommitment: frostPointSchema,
	bindingNonceCommitment: frostPointSchema,
});
// --- Signing Actions ---

const requestSignatureSchema = z.object({
	id: z.literal("sign_request"),
	groupId: groupIdSchema,
	message: hexBytes32Schema,
});

const registerNonceCommitmentsSchema = z.object({
	id: z.literal("sign_register_nonce_commitments"),
	groupId: groupIdSchema,
	nonceCommitmentsHash: hexBytes32Schema,
});

const revealNonceCommitmentsSchema = z.object({
	id: z.literal("sign_reveal_nonce_commitments"),
	signatureId: signatureIdSchema,
	nonceCommitments: publicNonceCommitmentsSchema,
	nonceProof: z.array(hexBytes32Schema),
});

const publishSignatureShareSchema = z.object({
	id: z.literal("sign_publish_signature_share"),
	signatureId: signatureIdSchema,
	signersRoot: hexBytes32Schema,
	signersProof: z.array(hexBytes32Schema),
	groupCommitment: frostPointSchema,
	commitmentShare: frostPointSchema,
	signatureShare: coercedBigIntSchema,
	lagrangeCoefficient: coercedBigIntSchema,
	callbackContext: hexDataSchema.optional(),
});

const signingActionSchema = z.discriminatedUnion("id", [
	requestSignatureSchema,
	registerNonceCommitmentsSchema,
	revealNonceCommitmentsSchema,
	publishSignatureShareSchema,
]);

// --- KeyGen Actions ---

export const startKeyGenSchema = z.object({
	id: z.literal("key_gen_start"),
	participants: hexBytes32Schema,
	count: z.int(),
	threshold: z.int(),
	context: hexBytes32Schema,
	participantId: participantIdSchema,
	commitments: z.array(frostPointSchema),
	pok: proofOfKnowledgeSchema,
	poap: proofOfAttestationParticipationSchema,
});

export const publishSecretSharesSchema = z.object({
	id: z.literal("key_gen_publish_secret_shares"),
	groupId: groupIdSchema,
	verificationShare: frostPointSchema,
	shares: z.array(coercedBigIntSchema),
});

export const keyGenComplainSchema = z.object({
	id: z.literal("key_gen_complain"),
	groupId: groupIdSchema,
	accused: participantIdSchema,
});

export const keyGenComplaintResponseSchema = z.object({
	id: z.literal("key_gen_complaint_response"),
	groupId: groupIdSchema,
	plaintiff: participantIdSchema,
	secretShare: coercedBigIntSchema,
});

export const keyGenConfirmSchema = z.object({
	id: z.literal("key_gen_confirm"),
	groupId: groupIdSchema,
	callbackContext: hexDataSchema.optional(),
});

export const keyGenActionSchema = z.discriminatedUnion("id", [
	startKeyGenSchema,
	publishSecretSharesSchema,
	keyGenComplainSchema,
	keyGenComplaintResponseSchema,
	keyGenConfirmSchema,
]);

// --- Consensus Actions ---

export const attestTransactionSchema = z.object({
	id: z.literal("consensus_attest_transaction"),
	epoch: coercedBigIntSchema,
	transactionHash: hexBytes32Schema,
	signatureId: signatureIdSchema,
});

export const stageEpochSchema = z.object({
	id: z.literal("consensus_stage_epoch"),
	proposedEpoch: coercedBigIntSchema,
	rolloverBlock: coercedBigIntSchema,
	groupId: groupIdSchema,
	signatureId: signatureIdSchema,
});

export const consensusActionSchema = z.discriminatedUnion("id", [attestTransactionSchema, stageEpochSchema]);

// --- Protocol Action & Retry ---

const protocolActionSchema = z.union([signingActionSchema, keyGenActionSchema, consensusActionSchema]);

const actionWithTimeoutSchema = z.intersection(
	protocolActionSchema,
	z.object({
		validUntil: z.number(),
	}),
);

export class SqliteActionQueue extends SqliteQueue<ActionWithTimeout> {
	constructor(database: Database) {
		super(actionWithTimeoutSchema, database, "actions");
	}
}

const ethTxSchema = z.object({
	to: checkedAddressSchema,
	value: coercedBigIntSchema,
	data: hexDataSchema,
	gas: coercedBigIntSchema.optional(),
});

const txStorageSchema = z
	.object({
		nonce: z.number(),
		transactionJson: z
			.string()
			.transform((arg) => JSON.parse(arg))
			.pipe(ethTxSchema),
		transactionHash: z.union([hexDataSchema, z.null()]),
		createdAt: z.number(),
	})
	.array();

export class SqliteTxStorage implements TransactionStorage {
	#db: Database;
	constructor(database: Database) {
		this.#db = database;

		this.#db.exec(`
			CREATE TABLE IF NOT EXISTS transaction_storage (
				nonce INTEGER PRIMARY KEY,
				transactionJson TEXT NOT NULL,
				transactionHash TEXT DEFAULT NULL,
				createdAt DATETIME DEFAULT (unixepoch())
			);
		`);
	}

	register(tx: EthTransactionData, minNonce: number): number {
		const transactionJson = JSON.stringify(tx, jsonReplacer);
		// If the minimum nonce is free lets use it otherwise use the next highest nonce
		const result = this.#db
			.prepare(`
			INSERT INTO transaction_storage (nonce, transactionJson)
			SELECT MAX($minNonce, COALESCE(MAX(nonce) + 1, $minNonce)), $transactionJson
			FROM transaction_storage
			RETURNING nonce;
		`)
			.run({
				minNonce,
				transactionJson,
			});
		return Number(result.lastInsertRowid);
	}

	setHash(nonce: number, txHash: Hex) {
		const updateStmt = this.#db.prepare(`
			UPDATE transaction_storage
			SET transactionHash = ?
			WHERE nonce = ?;
		`);
		updateStmt.run(txHash, nonce);
	}

	pending(createdDiff: number): (EthTransactionData & { nonce: number; hash: Hex | null })[] {
		const pendingTxsStmt = this.#db.prepare(`
			SELECT * FROM transaction_storage 
			WHERE createdAt <= (unixepoch() - ?);
		`);
		const pendingTxsResult = pendingTxsStmt.all(createdDiff);
		const pendingTxs = txStorageSchema.parse(pendingTxsResult);
		return pendingTxs.map((tx) => {
			return {
				...tx.transactionJson,
				nonce: tx.nonce,
				hash: tx.transactionHash,
			};
		});
	}

	setExecuted(nonce: number): void {
		const updateStmt = this.#db.prepare(`
			DELETE FROM transaction_storage
			WHERE nonce = ?;
		`);
		updateStmt.run(nonce);
	}

	setAllBeforeAsExecuted(nonce: number): number {
		const updateStmt = this.#db.prepare(`
			DELETE FROM transaction_storage
			WHERE nonce < ?;
		`);
		const result = updateStmt.run(nonce);
		return result.changes;
	}
}
