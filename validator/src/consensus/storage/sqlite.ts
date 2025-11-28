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
		`);

		this.#account = account;
		this.#db = db;

		// TODO: We can cache all our prepared SQL statements for performance
		// in the future.
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
			.prepare("SELECT id, address FROM group_participants WHERE group_id = ?")
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
				"SELECT id FROM group_participants WHERE group_id = ? AND commitments IS NULL",
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

	registerNonceTree(_tree: NonceTree): Hex {
		throw new Error("not implemented");
	}
	linkNonceTree(_groupId: GroupId, _chunk: bigint, _treeHash: Hex): void {
		throw new Error("not implemented");
	}
	nonceTree(_groupId: GroupId, _chunk: bigint): NonceTree {
		throw new Error("not implemented");
	}
	burnNonce(_groupId: GroupId, _chunk: bigint, _offset: bigint): void {
		throw new Error("not implemented");
	}
	registerSignatureRequest(
		_signatureId: SignatureId,
		_groupId: GroupId,
		_message: Hex,
		_signers: ParticipantId[],
		_sequence: bigint,
	): void {
		throw new Error("not implemented");
	}
	registerNonceCommitments(
		_signatureId: SignatureId,
		_signerId: ParticipantId,
		_nonceCommitments: PublicNonceCommitments,
	): void {
		throw new Error("not implemented");
	}
	checkIfNoncesComplete(_signatureId: SignatureId): boolean {
		throw new Error("not implemented");
	}
	missingNonces(_signatureId: SignatureId): ParticipantId[] {
		throw new Error("not implemented");
	}
	signingGroup(_signatureId: SignatureId): GroupId {
		throw new Error("not implemented");
	}
	signers(_signatureId: SignatureId): ParticipantId[] {
		throw new Error("not implemented");
	}
	message(_signatureId: SignatureId): Hex {
		throw new Error("not implemented");
	}
	sequence(_signatureId: SignatureId): bigint {
		throw new Error("not implemented");
	}
	nonceCommitmentsMap(
		_signatureId: SignatureId,
	): Map<ParticipantId, PublicNonceCommitments> {
		throw new Error("not implemented");
	}
}
