import Sqlite3, { type Database } from "better-sqlite3";
import { type Address, concat, type Hex } from "viem";
import { z } from "zod";
import {
	pointFromBytes,
	scalarFromBytes,
	scalarToBytes,
} from "../../frost/math.js";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	SignatureId,
} from "../../frost/types.js";
import {
	checkedAddressSchema,
	chunked,
	hexDataSchema,
} from "../../types/schemas.js";
import type { NonceTree, PublicNonceCommitments } from "../signing/nonces.js";
import type {
	GroupInfoStorage,
	KeyGenInfoStorage,
	Participant,
	SignatureRequestStorage,
} from "./types.js";

const dbIntegerSchema = z.int().transform((id) => BigInt(id));
const dbParticipantSchema = z.object({
	id: dbIntegerSchema,
	address: checkedAddressSchema,
});
const dbPointSchema = z.instanceof(Buffer).transform(pointFromBytes);
const dbScalarSchema = z.instanceof(Buffer).transform(scalarFromBytes);
const dbPointArraySchema = z
	.instanceof(Buffer)
	.transform(chunked(33, pointFromBytes));
const dbScalarArraySchema = z
	.instanceof(Buffer)
	.transform(chunked(32, scalarFromBytes));
const dbCommitmentsSchema = z.object({
	id: dbIntegerSchema,
	commitments: dbPointArraySchema,
});
const dbSecretShareSchema = z.object({
	id: dbIntegerSchema,
	secretShare: dbScalarSchema,
});
const dbNoncesCommitmentSchema = z.object({
	root: hexDataSchema,
	leaf: hexDataSchema,
	hiding: dbScalarSchema.nullable(),
	hidingCommitment: dbPointSchema,
	binding: dbScalarSchema.nullable(),
	bindingCommitment: dbPointSchema,
});
const dbSignatureCommitmentSchema = z.object({
	signer: dbIntegerSchema,
	hiding: dbPointSchema,
	binding: dbPointSchema,
});

interface ZodSchema<Output> {
	parse(data: unknown): Output;
}

export class SqliteStorage
	implements GroupInfoStorage, KeyGenInfoStorage, SignatureRequestStorage
{
	#account: Address;
	#db: Database;

	constructor(account: Address, path: string) {
		const db = new Sqlite3(path);
		db.exec(`
			CREATE TABLE IF NOT EXISTS groups(
				id TEXT NOT NULL,
				threshold INTEGER NOT NULL,
				public_key BLOB,
				PRIMARY KEY(id)
			);

			CREATE TABLE IF NOT EXISTS group_participants(
				group_id TEXT NOT NULL,
				id INTEGER NOT NULL,
				address TEXT NOT NULL,
				coefficients BLOB,
				commitments BLOB,
				verification_share BLOB,
				signing_share BLOB,
				PRIMARY KEY(group_id, id),
				FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE,
				UNIQUE(group_id,address)
			);

			CREATE TABLE IF NOT EXISTS group_secret_shares(
				group_id TEXT NOT NULL,
				address TEXT NOT NULL,
				from_participant INTEGER NOT NULL,
				secret_share BLOB NOT NULL,
				PRIMARY KEY(group_id, address, from_participant),
				FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
			);

			CREATE TABLE IF NOT EXISTS nonces_links(
				root TEXT NOT NULL,
				group_id TEXT NOT NULL,
				address TEXT NOT NULL,
				chunk INTEGER,
				PRIMARY KEY(root),
				FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
			);

			CREATE TABLE IF NOT EXISTS nonces(
				leaf TEXT NOT NULL,
				root TEXT NOT NULL,
				offset INTEGER NOT NULL,
				hiding BLOB,
		        hiding_commitment BLOB NOT NULL,
				binding BLOB,
				binding_commitment BLOB NOT NULL,
				PRIMARY KEY(leaf),
				FOREIGN KEY(root) REFERENCES nonces_links(root) ON DELETE CASCADE
			);

			CREATE TABLE IF NOT EXISTS signatures(
				id TEXT NOT NULL,
				group_id TEXT NOT NULL,
				message TEXT NOT NULL,
				sequence INTEGER NOT NULL,
				PRIMARY KEY(id),
				FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
			);

			CREATE TABLE IF NOT EXISTS signature_commitments(
				signature_id TEXT NOT NULL,
				signer INTEGER NOT NULL,
				hiding BLOB,
				binding BLOB,
				PRIMARY KEY(signature_id, signer),
				FOREIGN KEY(signature_id) REFERENCES signatures(id) ON DELETE CASCADE
			);
		`);

		this.#account = account;
		this.#db = db;

		// TODO: We can cache all our prepared SQL statements for performance
		// in the future. Additionally, there are a few indexes that we can add
		// to speed up SQL performance.
	}

	knownGroups(): GroupId[] {
		return this.#db
			.prepare("SELECT id FROM groups")
			.pluck(true)
			.all()
			.map((row) => hexDataSchema.parse(row));
	}

	registerGroup(
		groupId: GroupId,
		participants: readonly Participant[],
		threshold: bigint,
	): ParticipantId {
		// TODO: Computing the participant ID from inputs does not seem like the
		// responsibility of the client. Additionally, it is not possible to
		// correctly support multiple participant IDs managed by the same EOA.
		const participantId = participants.find(
			(p) => p.address === this.#account,
		)?.id;
		if (participantId === undefined) {
			throw Error(`Not part of Group ${groupId}!`);
		}

		const insertGroup = this.#db.prepare(
			"INSERT INTO groups (id, threshold) VALUES (?, ?)",
		);
		const insertParticipant = this.#db.prepare(
			"INSERT INTO group_participants (group_id, id, address) VALUES (?, ?, ?)",
		);
		this.#db.transaction(() => {
			const { changes } = insertGroup.run(groupId, threshold);
			if (changes !== 1) {
				throw new Error("group already exists");
			}
			for (const { id, address } of participants) {
				insertParticipant.run(groupId, id, address);
			}
		})();

		return participantId;
	}

	private setGroupColumn(
		groupId: GroupId,
		column: string,
		value: unknown,
	): void {
		const { changes } = this.#db
			.prepare(
				`UPDATE groups SET ${column} = ? WHERE id = ? AND ${column} IS NULL`,
			)
			.run(value, groupId);
		if (changes !== 1) {
			throw new Error("group not found or value already set");
		}
	}

	private setGroupThisParticipantColumn(
		groupId: GroupId,
		column: string,
		value: unknown,
	): void {
		const { changes } = this.#db
			.prepare(
				`UPDATE group_participants SET ${column} = ? WHERE group_id = ? AND address = ? AND ${column} IS NULL`,
			)
			.run(value, groupId, this.#account);
		if (changes !== 1) {
			throw new Error("group participant not found or value already set");
		}
	}

	private setGroupParticipantColumn(
		groupId: GroupId,
		participantId: ParticipantId,
		column: string,
		value: unknown,
	): void {
		const { changes } = this.#db
			.prepare(
				`UPDATE group_participants SET ${column} = ? WHERE group_id = ? AND id = ? AND ${column} IS NULL`,
			)
			.run(value, groupId, participantId);
		if (changes !== 1) {
			throw new Error("group participant not found or value already set");
		}
	}

	private getGroupColumn<T>(
		groupId: GroupId,
		column: string,
		schema: ZodSchema<T>,
	): T {
		const result = this.#db
			.prepare(`SELECT ${column} FROM groups WHERE id = ?`)
			.pluck(true)
			.get(groupId);
		if (result === undefined) {
			throw new Error("group not found");
		}
		// The interface expects "undefined" to signal a missing value instead
		// of null, so map that here.
		return schema.parse(result ?? undefined);
	}

	private getGroupParticipantColumn<T>(
		groupId: GroupId,
		participantId: ParticipantId,
		column: string,
		schema: ZodSchema<T>,
	): T {
		const result = this.#db
			.prepare(
				`SELECT ${column} FROM group_participants WHERE group_id = ? AND id = ?`,
			)
			.pluck(true)
			.get(groupId, participantId);
		if (result === undefined) {
			throw new Error("group participant not found");
		}
		// The interface expects "undefined" to signal a missing value instead
		// of null, so map that here.
		return schema.parse(result ?? undefined);
	}

	private getGroupThisParticipantColumn<T>(
		groupId: GroupId,
		column: string,
		schema: ZodSchema<T>,
	): T {
		const result = this.#db
			.prepare(
				`SELECT ${column} FROM group_participants WHERE group_id = ? AND address = ?`,
			)
			.pluck(true)
			.get(groupId, this.#account);
		if (result === undefined) {
			throw new Error("group participant not found");
		}
		// The interface expects "undefined" to signal a missing value instead
		// of null, so map that here.
		return schema.parse(result ?? undefined);
	}

	registerVerification(
		groupId: GroupId,
		groupPublicKey: FrostPoint,
		verificationShare: FrostPoint,
	): void {
		this.#db.transaction(() => {
			this.setGroupColumn(groupId, "public_key", groupPublicKey.toBytes());
			this.setGroupThisParticipantColumn(
				groupId,
				"verification_share",
				verificationShare.toBytes(),
			);
		})();
	}

	registerSigningShare(groupId: GroupId, signingShare: bigint): void {
		this.setGroupThisParticipantColumn(
			groupId,
			"signing_share",
			scalarToBytes(signingShare),
		);
	}

	participants(groupId: GroupId): readonly Participant[] {
		const result = this.#db
			.prepare(
				"SELECT id, address FROM group_participants WHERE group_id = ? ORDER BY id ASC",
			)
			.all(groupId)
			.map((row) => dbParticipantSchema.parse(row));
		// Note that registering a group requires there to be at least one
		// participant (as a corrolary to `this.#account` being included in the
		// participants list). This means that not finding any values here is
		// equivalent to the group not being registered.
		if (result.length === 0) {
			throw new Error("group not found");
		}
		return result;
	}

	participantId(groupId: GroupId): ParticipantId {
		return this.getGroupThisParticipantColumn(groupId, "id", dbIntegerSchema);
	}

	threshold(groupId: GroupId): bigint {
		return this.getGroupColumn(groupId, "threshold", dbIntegerSchema);
	}

	publicKey(groupId: GroupId): FrostPoint | undefined {
		return this.getGroupColumn(groupId, "public_key", dbPointSchema.optional());
	}

	verificationShare(groupId: GroupId): FrostPoint {
		return this.getGroupThisParticipantColumn(
			groupId,
			"verification_share",
			dbPointSchema,
		);
	}

	signingShare(groupId: GroupId): bigint | undefined {
		return this.getGroupThisParticipantColumn(
			groupId,
			"signing_share",
			dbScalarSchema.optional(),
		);
	}

	unregisterGroup(groupId: GroupId): void {
		this.#db.prepare("DELETE FROM groups WHERE id = ?").run(groupId);
	}

	registerKeyGen(groupId: GroupId, coefficients: readonly bigint[]): void {
		this.setGroupThisParticipantColumn(
			groupId,
			"coefficients",
			concat(coefficients.map(scalarToBytes)),
		);
	}

	registerCommitments(
		groupId: GroupId,
		participantId: ParticipantId,
		commitments: readonly FrostPoint[],
	): void {
		this.setGroupParticipantColumn(
			groupId,
			participantId,
			"commitments",
			concat(commitments.map((point) => point.toBytes())),
		);
	}

	registerSecretShare(
		groupId: GroupId,
		participantId: ParticipantId,
		share: bigint,
	): void {
		this.#db
			.prepare(
				"INSERT INTO group_secret_shares (group_id, address, from_participant, secret_share) VALUES (?, ?, ?, ?)",
			)
			.run(groupId, this.#account, participantId, scalarToBytes(share));
	}

	missingCommitments(groupId: GroupId): ParticipantId[] {
		return this.#db
			.prepare(
				"SELECT id FROM group_participants WHERE group_id = ? AND commitments IS NULL ORDER BY id ASC",
			)
			.pluck(true)
			.all(groupId)
			.map((row) => dbIntegerSchema.parse(row));
	}

	checkIfCommitmentsComplete(groupId: GroupId): boolean {
		const count = this.#db
			.prepare(
				"SELECT COUNT(*) FROM group_participants WHERE group_id = ? AND commitments IS NULL",
			)
			.pluck(true)
			.get(groupId);
		return count === 0;
	}

	missingSecretShares(groupId: GroupId): ParticipantId[] {
		return this.#db
			.prepare(`
				SELECT p.id FROM group_participants AS p
				LEFT JOIN group_secret_shares AS s ON s.group_id = p.group_id AND s.address = ? AND s.from_participant = p.id
				WHERE p.group_id = ? AND s.secret_share IS NULL
				ORDER BY id ASC
			`)
			.pluck(true)
			.all(this.#account, groupId)
			.map((row) => dbIntegerSchema.parse(row));
	}

	checkIfSecretSharesComplete(groupId: GroupId): boolean {
		const count = this.#db
			.prepare(`
				SELECT COUNT(*) FROM group_participants AS p
				LEFT JOIN group_secret_shares AS s ON s.group_id = p.group_id AND s.address = ? AND s.from_participant = p.id
				WHERE p.group_id = ? AND s.secret_share IS NULL
			`)
			.pluck(true)
			.get(this.#account, groupId);
		return count === 0;
	}

	encryptionKey(groupId: GroupId): bigint {
		return this.getGroupThisParticipantColumn(
			groupId,
			"SUBSTRING(coefficients, 1, 32)",
			dbScalarSchema,
		);
	}

	coefficients(groupId: GroupId): readonly bigint[] {
		return this.getGroupThisParticipantColumn(
			groupId,
			"coefficients",
			dbScalarArraySchema,
		);
	}

	commitments(
		groupId: GroupId,
		participantId: ParticipantId,
	): readonly FrostPoint[] {
		return this.getGroupParticipantColumn(
			groupId,
			participantId,
			"commitments",
			dbPointArraySchema,
		);
	}

	commitmentsMap(groupId: GroupId): Map<ParticipantId, readonly FrostPoint[]> {
		return new Map(
			this.#db
				.prepare(
					"SELECT id, commitments FROM group_participants WHERE group_id = ? AND commitments IS NOT NULL",
				)
				.all(groupId)
				.map((row) => {
					const { id, commitments } = dbCommitmentsSchema.parse(row);
					return [id, commitments];
				}),
		);
	}

	secretSharesMap(groupId: GroupId): Map<ParticipantId, bigint> {
		return new Map(
			this.#db
				.prepare(`
					SELECT from_participant as id, secret_share as secretShare
					FROM group_secret_shares
					WHERE group_id = ? AND address = ? AND secret_share IS NOT NULL
				`)
				.all(groupId, this.#account)
				.map((row) => {
					const { id, secretShare } = dbSecretShareSchema.parse(row);
					return [id, secretShare];
				}),
		);
	}

	clearKeyGen(groupId: GroupId): void {
		const deleteCoefficientsAndCommitments = this.#db.prepare(
			"UPDATE group_participants SET coefficients = NULL, commitments = NULL WHERE group_id = ?",
		);
		const deleteSecretShares = this.#db.prepare(
			"DELETE FROM group_secret_shares WHERE group_id = ?",
		);
		this.#db.transaction(() => {
			deleteCoefficientsAndCommitments.run(groupId);
			deleteSecretShares.run(groupId);
		})();
	}

	registerNonceTree(groupId: GroupId, tree: NonceTree): Hex {
		const insertNoncesLink = this.#db.prepare(
			"INSERT INTO nonces_links (root, group_id, address) VALUES (?, ?, ?)",
		);
		const insertNoncesLeaf = this.#db.prepare(
			"INSERT INTO nonces (leaf, root, offset, hiding, hiding_commitment, binding, binding_commitment) VALUES (?, ?, ?, ?, ?, ?, ?)",
		);
		this.#db.transaction(() => {
			insertNoncesLink.run(tree.root, groupId, this.#account);
			for (let offset = 0; offset < tree.commitments.length; offset++) {
				insertNoncesLeaf.run(
					tree.leaves[offset],
					tree.root,
					offset,
					scalarToBytes(tree.commitments[offset].hidingNonce),
					tree.commitments[offset].hidingNonceCommitment.toBytes(),
					scalarToBytes(tree.commitments[offset].bindingNonce),
					tree.commitments[offset].bindingNonceCommitment.toBytes(),
				);
			}
		})();
		// TODO: feels like a code-smell that we return an input parameter.
		return tree.root;
	}

	linkNonceTree(groupId: GroupId, chunk: bigint, treeHash: Hex): void {
		const { changes } = this.#db
			.prepare(
				"UPDATE nonces_links SET chunk = ? WHERE root = ? AND group_id = ? AND chunk is NULL",
			)
			.run(chunk, treeHash, groupId);
		if (changes !== 1) {
			throw new Error("nonces root not found or already linked to chunk");
		}
	}

	nonceTree(groupId: GroupId, chunk: bigint): NonceTree {
		const nonces = this.#db
			.prepare(`
				SELECT
					root,
					leaf,
					hiding,
					hiding_commitment AS hidingCommitment,
					binding,
					binding_commitment AS bindingCommitment
				FROM nonces
				WHERE root = (
					SELECT l.root FROM nonces_links AS l
					WHERE l.group_id = ? AND l.address = ? AND l.chunk = ?
				)
				ORDER BY offset ASC
			`)
			.all(groupId, this.#account, chunk)
			.map((row) => dbNoncesCommitmentSchema.parse(row));
		if (nonces.length === 0) {
			throw new Error("nonce tree not found");
		}
		return {
			root: nonces[0].root,
			leaves: nonces.map((n) => n.leaf),
			commitments: nonces.map((n) => ({
				hidingNonce: n.hiding ?? 0n,
				hidingNonceCommitment: n.hidingCommitment,
				bindingNonce: n.binding ?? 0n,
				bindingNonceCommitment: n.bindingCommitment,
			})),
		};
	}

	burnNonce(groupId: GroupId, chunk: bigint, offset: bigint): void {
		const { changes } = this.#db
			.prepare(`
				UPDATE nonces
				SET hiding = NULL, binding = NULL
				WHERE root = (
					SELECT l.root FROM nonces_links AS l
					WHERE l.group_id = ? AND l.address = ? AND l.chunk = ?
				)
				AND offset = ? AND hiding IS NOT NULL AND binding IS NOT NULL
			`)
			.run(groupId, this.#account, chunk, offset);
		if (changes !== 1) {
			throw new Error("nonce not found or already burned");
		}
	}

	private getSignatureColumn<T>(
		signatureId: SignatureId,
		column: string,
		schema: ZodSchema<T>,
	): T {
		const result = this.#db
			.prepare(`SELECT ${column} FROM signatures WHERE id = ?`)
			.pluck(true)
			.get(signatureId);
		if (result === undefined) {
			throw new Error("signature not found");
		}
		return schema.parse(result);
	}

	registerSignatureRequest(
		signatureId: SignatureId,
		groupId: GroupId,
		message: Hex,
		signers: ParticipantId[],
		sequence: bigint,
	): void {
		if (signers.length === 0) {
			throw new Error("signature with no signers");
		}

		const insertSignature = this.#db.prepare(
			"INSERT INTO signatures (id, group_id, message, sequence) VALUES (?, ?, ?, ?)",
		);
		const insertSignatureCommitment = this.#db.prepare(
			"INSERT INTO signature_commitments (signature_id, signer) VALUES (?, ?)",
		);
		this.#db.transaction(() => {
			insertSignature.run(signatureId, groupId, message, sequence);
			for (const signer of signers) {
				insertSignatureCommitment.run(signatureId, signer);
			}
		})();
	}

	registerNonceCommitments(
		signatureId: SignatureId,
		signerId: ParticipantId,
		nonceCommitments: PublicNonceCommitments,
	): void {
		const { changes } = this.#db
			.prepare(`
				UPDATE signature_commitments SET hiding = ?, binding = ?
				WHERE signature_id = ? AND signer = ? AND hiding IS NULL AND binding IS NULL
			`)
			.run(
				nonceCommitments.hidingNonceCommitment.toBytes(),
				nonceCommitments.bindingNonceCommitment.toBytes(),
				signatureId,
				signerId,
			);
		if (changes !== 1) {
			throw new Error("signature commitment not found or already registered");
		}
	}

	checkIfNoncesComplete(signatureId: SignatureId): boolean {
		const count = this.#db
			.prepare(
				"SELECT COUNT(*) FROM signature_commitments WHERE signature_id = ? AND (hiding IS NULL OR binding IS NULL)",
			)
			.pluck(true)
			.get(signatureId);
		return count === 0;
	}

	missingNonces(signatureId: SignatureId): ParticipantId[] {
		return this.#db
			.prepare(
				"SELECT signer FROM signature_commitments WHERE signature_id = ? AND (hiding IS NULL OR binding IS NULL) ORDER BY signer ASC",
			)
			.pluck(true)
			.all(signatureId)
			.map((row) => dbIntegerSchema.parse(row));
	}

	signingGroup(signatureId: SignatureId): GroupId {
		return this.getSignatureColumn(signatureId, "group_id", hexDataSchema);
	}

	signers(signatureId: SignatureId): ParticipantId[] {
		const result = this.#db
			.prepare(
				"SELECT signer FROM signature_commitments WHERE signature_id = ? ORDER BY signer ASC",
			)
			.pluck(true)
			.all(signatureId)
			.map((row) => dbIntegerSchema.parse(row));
		if (result.length === 0) {
			throw new Error("signature not found");
		}
		return result;
	}

	message(signatureId: SignatureId): Hex {
		return this.getSignatureColumn(signatureId, "message", hexDataSchema);
	}

	sequence(signatureId: SignatureId): bigint {
		return this.getSignatureColumn(signatureId, "sequence", dbIntegerSchema);
	}

	nonceCommitmentsMap(
		signatureId: SignatureId,
	): Map<ParticipantId, PublicNonceCommitments> {
		return new Map(
			this.#db
				.prepare(`
					SELECT signer, hiding, binding FROM signature_commitments
					WHERE signature_id = ? AND (hiding IS NOT NULL AND binding IS NOT NULL)
				`)
				.all(signatureId)
				.map((row) => {
					const { signer, hiding, binding } =
						dbSignatureCommitmentSchema.parse(row);
					return [
						signer,
						{ hidingNonceCommitment: hiding, bindingNonceCommitment: binding },
					];
				}),
		);
	}
}
