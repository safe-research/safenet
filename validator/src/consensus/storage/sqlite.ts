import Sqlite3, { type Database } from "better-sqlite3";
import type { Address, Hex } from "viem";
import { z } from "zod";
import { pointFromHex, scalarFromHex, scalarToHex } from "../../frost/math.js";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	SignatureId,
} from "../../frost/types.js";
import { checkedAddressSchema, hexDataSchema } from "../../types/schemas.js";
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
const dbPointSchema = z.hex().transform(pointFromHex);
const dbScalarSchema = z.hex().transform(scalarFromHex);

interface ZodSchema<Output> {
	parse(data: unknown): Output;
}

export class SqliteStorage
	implements KeyGenInfoStorage, GroupInfoStorage, SignatureRequestStorage
{
	#account: Address;
	#db: Database;

	constructor(account: Address, path: string) {
		const db = new Sqlite3(path);
		db.exec(`
			CREATE TABLE IF NOT EXISTS groups(
				id TEXT NOT NULL,
				threshold INTEGER NOT NULL,
				public_key TEXT,
				verification_share TEXT,
				signing_share TEXT,
				PRIMARY KEY (id)
			);

			CREATE TABLE IF NOT EXISTS group_participants(
				group_id TEXT NOT NULL,
				id INTEGER NOT NULL,
				address TEXT NOT NULL,
				PRIMARY KEY (group_id, id)
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
		// responsibility of the client.
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
			insertGroup.run(groupId, threshold);
			for (const { id, address } of participants) {
				insertParticipant.run(groupId, id, address);
			}
		})();

		return participantId;
	}

	registerVerification(
		groupId: GroupId,
		groupPublicKey: FrostPoint,
		verificationShare: FrostPoint,
	): void {
		const { changes } = this.#db
			.prepare(
				"UPDATE groups SET public_key = ?, verification_share = ? WHERE id = ? AND public_key IS NULL",
			)
			.run(groupPublicKey.toHex(), verificationShare.toHex(), groupId);
		if (changes < 1) {
			throw new Error("group not found");
		}
	}

	registerSigningShare(groupId: GroupId, signingShare: bigint): void {
		const { changes } = this.#db
			.prepare(
				"UPDATE groups SET signing_share = ? WHERE id = ? AND signing_share IS NULL",
			)
			.run(scalarToHex(signingShare), groupId);
		if (changes < 1) {
			throw new Error("group not found");
		}
	}

	participantId(groupId: GroupId): ParticipantId {
		const result = this.#db
			.prepare(
				"SELECT id FROM group_participants WHERE group_id = ? AND address = ?",
			)
			.pluck(true)
			.get(groupId, this.#account);
		if (result === undefined) {
			throw new Error("participant not in group");
		}

		return dbIntegerSchema.parse(result);
	}

	private groupColumn<T>(
		groupId: GroupId,
		column: string,
		schema: ZodSchema<T>,
	): T {
		const result = this.#db
			.prepare(`SELECT ${column} as hex FROM groups WHERE id = ?`)
			.pluck(true)
			.get(groupId);
		if (result === undefined) {
			throw new Error("group not found");
		}
		// The interface expects "undefined" to signal a missing value instead
		// of null, so map that here.
		return schema.parse(result ?? undefined);
	}

	participants(groupId: GroupId): readonly Participant[] {
		// Make sure that the group is present!
		this.groupColumn(groupId, "id", hexDataSchema);
		return this.#db
			.prepare("SELECT id, address FROM group_participants WHERE group_id = ?")
			.all(groupId)
			.map((row) => dbParticipantSchema.parse(row));
	}

	threshold(groupId: GroupId): bigint {
		return this.groupColumn(groupId, "threshold", dbIntegerSchema);
	}

	publicKey(groupId: GroupId): FrostPoint | undefined {
		return this.groupColumn(groupId, "public_key", dbPointSchema.optional());
	}

	verificationShare(groupId: GroupId): FrostPoint {
		return this.groupColumn(groupId, "verification_share", dbPointSchema);
	}

	signingShare(groupId: GroupId): bigint | undefined {
		return this.groupColumn(
			groupId,
			"signing_share",
			dbScalarSchema.optional(),
		);
	}

	unregisterGroup(groupId: GroupId): void {
		this.#db.transaction(() => {
			this.#db.prepare("DELETE FROM groups WHERE id = ?").run(groupId);
			this.#db
				.prepare("DELETE FROM group_participants WHERE group_id = ?")
				.run(groupId);
		})();
	}

	registerKeyGen(_groupId: GroupId, _coefficients: readonly bigint[]): void {
		throw new Error("not implemented");
	}
	registerCommitments(
		_groupId: GroupId,
		_participantId: ParticipantId,
		_commitments: readonly FrostPoint[],
	): void {
		throw new Error("not implemented");
	}
	registerSecretShare(
		_groupId: GroupId,
		_participantId: ParticipantId,
		_share: bigint,
	): void {
		throw new Error("not implemented");
	}
	checkIfCommitmentsComplete(_groupId: GroupId): boolean {
		throw new Error("not implemented");
	}
	checkIfSecretSharesComplete(_groupId: GroupId): boolean {
		throw new Error("not implemented");
	}

	encryptionKey(_groupId: GroupId): bigint {
		throw new Error("not implemented");
	}
	coefficients(_groupId: GroupId): readonly bigint[] {
		throw new Error("not implemented");
	}
	commitments(
		_groupId: GroupId,
		_participantId: ParticipantId,
	): readonly FrostPoint[] {
		throw new Error("not implemented");
	}
	commitmentsMap(_groupId: GroupId): Map<ParticipantId, readonly FrostPoint[]> {
		throw new Error("not implemented");
	}
	secretSharesMap(_groupId: GroupId): Map<ParticipantId, bigint> {
		throw new Error("not implemented");
	}
	clearKeyGen(_groupId: GroupId): void {
		throw new Error("not implemented");
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
