import type { Database } from "better-sqlite3";
import { type Address, concat, type Hex } from "viem";
import { z } from "zod";
import { pointFromBytes, scalarFromBytes, scalarToBytes } from "../../frost/math.js";
import type { FrostPoint, GroupId, ParticipantId, SignatureId } from "../../frost/types.js";
import { checkedAddressSchema, chunked, hexBytes32Schema } from "../../types/schemas.js";
import type { NonceTree, PublicNonceCommitments } from "../signing/nonces.js";
import type { GroupInfoStorage, KeyGenInfoStorage, Participant, SignatureRequestStorage } from "./types.js";

interface ZodSchema<Output> {
	parse(data: unknown): Output;
}

const dbIntegerSchema = z.int().transform((id) => BigInt(id));
const dbParticipantSchema = z.object({
	id: dbIntegerSchema,
	address: checkedAddressSchema,
});
const dbPointSchema = z.instanceof(Buffer).transform(pointFromBytes);
const dbScalarSchema = z.instanceof(Buffer).transform(scalarFromBytes);
const dbPointArraySchema = z.instanceof(Buffer).transform(chunked(33, pointFromBytes));
const dbScalarArraySchema = z.instanceof(Buffer).transform(chunked(32, scalarFromBytes));
const dbCommitmentsSchema = z.object({
	id: dbIntegerSchema,
	commitments: dbPointArraySchema,
});
const dbSecretShareSchema = z.object({
	id: dbIntegerSchema,
	secretShare: dbScalarSchema,
});
const dbNoncesCommitmentSchema = z.object({
	root: hexBytes32Schema,
	leaf: hexBytes32Schema,
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
const dbEmptyListSchema = z.array(z.null()).length(1);
const dbList = <T>(result: unknown[], schema: ZodSchema<T>): T[] => {
	// NOTE: For certain queries, we use a `LEFT JOIN` on the primary ID (for
	// example group ID or signature ID) so that we return `[]` in case of
	// a missing primary ID, a `[null]` in case there is a primary ID but with
	// no associated values (for example, no missing nonces), and `[...]`
	// otherwise. This allows is us to disambiguate between "_ not found" errors
	// and empty lists.
	if (result.length === 0) {
		throw new Error("not found");
	}
	if (dbEmptyListSchema.safeParse(result).success) {
		return [];
	}
	return result.map((row) => schema.parse(row));
};
const dbEmptyMapSchema = z.array(z.record(z.string(), z.null())).length(1);
const dbMap = <T, K, V>(result: unknown[], schema: ZodSchema<T>, f: (value: T) => [K, V]): Map<K, V> => {
	// NOTE: We apply the same `LEFT JOIN` trick for maps as we do for lists.
	if (result.length === 0) {
		throw new Error("not found");
	}
	if (dbEmptyMapSchema.safeParse(result).success) {
		return new Map();
	}
	return new Map(result.map((row) => f(schema.parse(row))));
};

export class SqliteClientStorage implements GroupInfoStorage, KeyGenInfoStorage, SignatureRequestStorage {
	#account: Address;
	#db: Database;

	constructor(account: Address, database: Database) {
		this.#account = account;
		this.#db = database;

		this.#db.exec(`
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

		// TODO: We can cache all our prepared SQL statements for performance
		// in the future. Additionally, there are a few indexes that we can add
		// to speed up SQL performance.
	}

	accountAddress(): Address {
		return this.#account;
	}

	knownGroups(): GroupId[] {
		return this.#db
			.prepare("SELECT id FROM groups")
			.pluck(true)
			.all()
			.map((row) => hexBytes32Schema.parse(row));
	}

	registerGroup(groupId: GroupId, participants: readonly Participant[], threshold: bigint): ParticipantId {
		// TODO: Computing the participant ID from inputs does not seem like the
		// responsibility of the client. Additionally, it is not possible to
		// correctly support multiple participant IDs managed by the same EOA.
		const participantId = participants.find((p) => p.address === this.#account)?.id;
		if (participantId === undefined) {
			throw new Error(`Not part of Group ${groupId}!`);
		}

		const insertGroup = this.#db.prepare("INSERT INTO groups (id, threshold) VALUES (?, ?)");
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

	private setGroupColumn(groupId: GroupId, column: string, value: unknown): void {
		const { changes } = this.#db
			.prepare(`UPDATE groups SET ${column} = ? WHERE id = ? AND ${column} IS NULL`)
			.run(value, groupId);
		if (changes !== 1) {
			throw new Error("group not found or value already set");
		}
	}

	private setGroupThisParticipantColumn(groupId: GroupId, column: string, value: unknown): void {
		const { changes } = this.#db
			.prepare(`UPDATE group_participants SET ${column} = ? WHERE group_id = ? AND address = ? AND ${column} IS NULL`)
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
			.prepare(`UPDATE group_participants SET ${column} = ? WHERE group_id = ? AND id = ? AND ${column} IS NULL`)
			.run(value, groupId, participantId);
		if (changes !== 1) {
			throw new Error("group participant not found or value already set");
		}
	}

	private getGroupColumn<T>(groupId: GroupId, column: string, schema: ZodSchema<T>): T {
		const result = this.#db.prepare(`SELECT ${column} FROM groups WHERE id = ?`).pluck(true).get(groupId);
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
			.prepare(`SELECT ${column} FROM group_participants WHERE group_id = ? AND id = ?`)
			.pluck(true)
			.get(groupId, participantId);
		if (result === undefined) {
			throw new Error("group participant not found");
		}
		// The interface expects "undefined" to signal a missing value instead
		// of null, so map that here.
		return schema.parse(result ?? undefined);
	}

	private getGroupThisParticipantColumn<T>(groupId: GroupId, column: string, schema: ZodSchema<T>): T {
		const result = this.#db
			.prepare(`SELECT ${column} FROM group_participants WHERE group_id = ? AND address = ?`)
			.pluck(true)
			.get(groupId, this.#account);
		if (result === undefined) {
			throw new Error("group participant not found");
		}
		// The interface expects "undefined" to signal a missing value instead
		// of null, so map that here.
		return schema.parse(result ?? undefined);
	}

	registerVerification(groupId: GroupId, groupPublicKey: FrostPoint, verificationShare: FrostPoint): void {
		this.#db.transaction(() => {
			this.setGroupColumn(groupId, "public_key", groupPublicKey.toBytes());
			this.setGroupThisParticipantColumn(groupId, "verification_share", verificationShare.toBytes());
		})();
	}

	registerSigningShare(groupId: GroupId, signingShare: bigint): void {
		this.setGroupThisParticipantColumn(groupId, "signing_share", scalarToBytes(signingShare));
	}

	participants(groupId: GroupId): readonly Participant[] {
		const result = this.#db
			.prepare("SELECT id, address FROM group_participants WHERE group_id = ? ORDER BY id ASC")
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
		return this.getGroupThisParticipantColumn(groupId, "verification_share", dbPointSchema);
	}

	signingShare(groupId: GroupId): bigint | undefined {
		return this.getGroupThisParticipantColumn(groupId, "signing_share", dbScalarSchema.optional());
	}

	unregisterGroup(groupId: GroupId): void {
		this.#db.prepare("DELETE FROM groups WHERE id = ?").run(groupId);
	}

	registerKeyGen(groupId: GroupId, coefficients: readonly bigint[]): void {
		this.setGroupThisParticipantColumn(groupId, "coefficients", concat(coefficients.map(scalarToBytes)));
	}

	registerCommitments(groupId: GroupId, participantId: ParticipantId, commitments: readonly FrostPoint[]): void {
		this.setGroupParticipantColumn(
			groupId,
			participantId,
			"commitments",
			concat(commitments.map((point) => point.toBytes())),
		);
	}

	registerSecretShare(groupId: GroupId, participantId: ParticipantId, share: bigint): void {
		this.#db
			.prepare(
				"INSERT INTO group_secret_shares (group_id, address, from_participant, secret_share) VALUES (?, ?, ?, ?)",
			)
			.run(groupId, this.#account, participantId, scalarToBytes(share));
	}

	missingCommitments(groupId: GroupId): ParticipantId[] {
		// Use a `LEFT JOIN` for this query; see documentation in `dbList` for
		// more information on the rationale behind structuring it this way.
		//
		// Because we are selecting the `groups` table and using a `LEFT JOIN`,
		// then if the group with `groupId` exists then the query will return
		// at least one row regardless of whether or not there are matching
		// participants (with a `group_participants.id` value of `NULL` in case
		// there are no rows from the `group_participants` table to join on).
		// If there are no matching groups with `groupId`, then the select will
		// return no rows. This, critically, allows us to differentiate between
		// missing groups, and groups with no missing commitments:
		//
		// - returns `[]` in case there is no rows in `groups` with `groupId`.
		// - returns `[null]` in case there is a row in `groups` with `groupId`
		//   but no participants in `group_participants` with no commitments.
		// - returns `[1, 2, ...]` in case there are missing commitments.

		return dbList(
			this.#db
				.prepare(`
					SELECT p.id
					FROM groups AS g
					LEFT JOIN group_participants AS p
					ON p.group_id = g.id AND p.commitments IS NULL
					WHERE g.id = ?
					ORDER BY p.id ASC
				`)
				.pluck(true)
				.all(groupId),
			dbIntegerSchema,
		);
	}

	checkIfCommitmentsComplete(groupId: GroupId): boolean {
		// Select based on the existence of a row in `groups` with `groupId`,
		// so that we differentiate between "there is no group" (which will
		// cause the query to return `NULL`) and "there are no missing
		// commitments" (which will cause the query to return `0`).
		//
		// This is essentially an atomic version of:
		// 1. Check that there exists a group in `groups` with `groupId`
		//    a. If yes, then return whether or not there exists rows in
		//       `group_participants` where the commitments is still `NULL`,
		//       returning `1` in case there are, meaning that the commitments
		//       **are not** complete; and `0` in case there are not, meaning
		//       the commitments **are** complete.
		//    b. If not, return `NULL` to indicate that there is no group.

		const exists = this.#db
			.prepare(`
				SELECT CASE
					WHEN EXISTS (SELECT id FROM groups WHERE id = ?) THEN EXISTS (
						SELECT 1
						FROM group_participants
						WHERE group_id = ? AND commitments IS NULL
					)
					ELSE NULL
				END
			`)
			.pluck(true)
			.get(groupId, groupId);
		if (exists === null) {
			throw new Error("group not found");
		}
		return exists === 0;
	}

	missingSecretShares(groupId: GroupId): ParticipantId[] {
		// Use the `LEFT JOIN` trick described in `dbList` and in the
		// `missingCommitments` query. Note that the query has some additional
		// complexity around secret shares and participants being split into
		// two tables.

		return dbList(
			this.#db
				.prepare(`
					WITH group_participant_secret_shares AS (
						SELECT p.group_id, p.id, s.secret_share
						FROM group_participants AS p
						LEFT JOIN group_secret_shares AS s
						ON s.group_id = p.group_id AND s.address = ? AND s.from_participant = p.id
					)
					SELECT t.id FROM groups AS g
					LEFT JOIN group_participant_secret_shares AS t
					ON t.group_id = g.id AND t.secret_share IS NULL
					WHERE g.id = ?
					ORDER BY t.id ASC
				`)
				.pluck(true)
				.all(this.#account, groupId),
			dbIntegerSchema,
		);
	}

	checkIfSecretSharesComplete(groupId: GroupId): boolean {
		// Differentiate between "group not found" and "no missing secret
		// shares"; see `checkIfCommitmentsComplete` for more information.

		const exists = this.#db
			.prepare(`
				SELECT CASE
					WHEN EXISTS (SELECT id FROM groups WHERE id = ?) THEN EXISTS (
						SELECT 1
						FROM group_participants AS p
				 		LEFT JOIN group_secret_shares AS s
				 		ON s.group_id = p.group_id AND s.address = ? AND s.from_participant = p.id
				 		WHERE p.group_id = ? AND s.secret_share IS NULL
					)
					ELSE NULL
				END
			`)
			.pluck(true)
			.get(groupId, this.#account, groupId);
		if (exists === null) {
			throw new Error("group not found");
		}
		return exists === 0;
	}

	encryptionKey(groupId: GroupId): bigint {
		return this.getGroupThisParticipantColumn(groupId, "SUBSTRING(coefficients, 1, 32)", dbScalarSchema);
	}

	coefficients(groupId: GroupId): readonly bigint[] {
		return this.getGroupThisParticipantColumn(groupId, "coefficients", dbScalarArraySchema);
	}

	commitments(groupId: GroupId, participantId: ParticipantId): readonly FrostPoint[] {
		return this.getGroupParticipantColumn(groupId, participantId, "commitments", dbPointArraySchema);
	}

	commitmentsMap(groupId: GroupId): Map<ParticipantId, readonly FrostPoint[]> {
		// Use the `LEFT JOIN` trick described in `dbList` and in the
		// `missingCommitments` query, adapted to mappings.

		return dbMap(
			this.#db
				.prepare(`
					SELECT p.id, p.commitments
					FROM groups AS g
					LEFT JOIN group_participants AS p
					ON p.group_id = g.id AND p.commitments IS NOT NULL
					WHERE g.id = ?
				`)
				.all(groupId),
			dbCommitmentsSchema,
			({ id, commitments }) => [id, commitments],
		);
	}

	secretSharesMap(groupId: GroupId): Map<ParticipantId, bigint> {
		// Use the `LEFT JOIN` trick decribed in `dbList` and in the
		// `missingSecretShares` query, adapted to mappings.

		return dbMap(
			this.#db
				.prepare(`
					SELECT s.from_participant AS id, s.secret_share AS secretShare
					FROM groups AS g
					LEFT JOIN group_secret_shares AS s
					ON s.group_id = g.id AND s.address = ? AND s.secret_share IS NOT NULL
					WHERE g.id = ?
				`)
				.all(this.#account, groupId),
			dbSecretShareSchema,
			({ id, secretShare }) => [id, secretShare],
		);
	}

	clearKeyGen(groupId: GroupId): void {
		const deleteCoefficientsAndCommitments = this.#db.prepare(
			"UPDATE group_participants SET coefficients = NULL, commitments = NULL WHERE group_id = ?",
		);
		const deleteSecretShares = this.#db.prepare("DELETE FROM group_secret_shares WHERE group_id = ?");
		this.#db.transaction(() => {
			deleteCoefficientsAndCommitments.run(groupId);
			deleteSecretShares.run(groupId);
		})();
	}

	registerNonceTree(groupId: GroupId, tree: NonceTree): Hex {
		const insertNoncesLink = this.#db.prepare("INSERT INTO nonces_links (root, group_id, address) VALUES (?, ?, ?)");
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
			.prepare("UPDATE nonces_links SET chunk = ? WHERE root = ? AND group_id = ? AND chunk is NULL")
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

	private getSignatureColumn<T>(signatureId: SignatureId, column: string, schema: ZodSchema<T>): T {
		const result = this.#db.prepare(`SELECT ${column} FROM signatures WHERE id = ?`).pluck(true).get(signatureId);
		if (result === undefined) {
			throw new Error("signature request not found");
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
		// Differentiate between "signature request not found" and "no missing
		// nonces commitments"; see `checkIfCommitmentsComplete` for more
		// information.

		const exists = this.#db
			.prepare(`
				SELECT CASE
					WHEN EXISTS (SELECT id FROM signatures WHERE id = ?) THEN EXISTS (
						SELECT 1
						FROM signature_commitments
						WHERE signature_id = ?
						AND (hiding IS NULL OR binding IS NULL)
					)
					ELSE NULL
				END
			`)
			.pluck(true)
			.get(signatureId, signatureId);
		if (exists === null) {
			throw new Error("signature request not found");
		}
		return exists === 0;
	}

	missingNonces(signatureId: SignatureId): ParticipantId[] {
		// Use the `LEFT JOIN` trick described in `dbList` and in the
		// `missingCommitments` query, adapted for signature requests.

		return dbList(
			this.#db
				.prepare(`
					SELECT c.signer
					FROM signatures AS s
					LEFT JOIN signature_commitments AS c
					ON c.signature_id = s.id AND (c.hiding IS NULL OR c.binding IS NULL)
					WHERE s.id = ?
					ORDER BY c.signer ASC
				`)
				.pluck(true)
				.all(signatureId),
			dbIntegerSchema,
		);
	}

	signingGroup(signatureId: SignatureId): GroupId {
		return this.getSignatureColumn(signatureId, "group_id", hexBytes32Schema);
	}

	signers(signatureId: SignatureId): ParticipantId[] {
		const result = this.#db
			.prepare("SELECT signer FROM signature_commitments WHERE signature_id = ? ORDER BY signer ASC")
			.pluck(true)
			.all(signatureId);
		if (result.length === 0) {
			throw new Error("signature request not found");
		}
		return result.map((row) => dbIntegerSchema.parse(row));
	}

	message(signatureId: SignatureId): Hex {
		return this.getSignatureColumn(signatureId, "message", hexBytes32Schema);
	}

	sequence(signatureId: SignatureId): bigint {
		return this.getSignatureColumn(signatureId, "sequence", dbIntegerSchema);
	}

	nonceCommitmentsMap(signatureId: SignatureId): Map<ParticipantId, PublicNonceCommitments> {
		// Use the `LEFT JOIN` trick described in `dbList` and in the
		// `missingNonces` query, adapted to mappings.

		return dbMap(
			this.#db
				.prepare(`
					SELECT c.signer, c.hiding, c.binding
					FROM signatures AS s
					LEFT JOIN signature_commitments AS c
					ON c.signature_id = s.id AND c.hiding IS NOT NULL AND c.binding IS NOT NULL
					WHERE s.id = ?
				`)
				.all(signatureId),
			dbSignatureCommitmentSchema,
			({ signer, hiding, binding }) => [
				signer,
				{
					hidingNonceCommitment: hiding,
					bindingNonceCommitment: binding,
				},
			],
		);
	}
}
