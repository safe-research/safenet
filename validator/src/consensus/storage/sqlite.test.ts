import Sqlite3 from "better-sqlite3";
import type { Address } from "viem";
import { describe, expect, it } from "vitest";
import { g } from "../../frost/math.js";
import { SqliteClientStorage } from "./sqlite.js";

const groups = [`0x${"ff".repeat(31)}01`, `0x${"ff".repeat(31)}02`] as const;

const participants = [
	{ id: 1n, address: "0x0000000000000000000000000000000000000001" },
	{ id: 2n, address: "0x0000000000000000000000000000000000000002" },
	{ id: 3n, address: "0x0000000000000000000000000000000000000003" },
] as const;

const sortedEntries = <T>(m: Map<bigint, T>): [bigint, T][] => {
	return [...m.entries()].sort(([a], [b]) => Number(a - b));
};

describe("sqlite", () => {
	const testStorage = (address: Address) => new SqliteClientStorage(address, new Sqlite3(":memory:"));

	describe("GroupInfoStorage", () => {
		it("should register groups with participants", () => {
			const storage = testStorage(participants[1].address);

			expect(storage.knownGroups()).toEqual([]);
			expect(() => storage.participantId(groups[0])).toThrowError();
			expect(() => storage.participants(groups[0])).toThrowError();
			expect(() => storage.threshold(groups[0])).toThrowError();

			const participantId = storage.registerGroup(groups[0], participants, 2n);
			expect(participantId).toBe(participants[1].id);

			expect(storage.knownGroups()).toEqual([groups[0]]);
			expect(storage.participantId(groups[0])).toBe(participants[1].id);
			expect(storage.participants(groups[0])).toEqual(participants);
			expect(storage.threshold(groups[0])).toBe(2n);
		});

		it("should register group public key and verification share", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.publicKey(groups[0])).toThrowError();
			expect(() => storage.verificationShare(groups[0])).toThrowError();
			expect(() => storage.registerVerification(groups[0], g(1n), g(2n))).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			expect(storage.publicKey(groups[0])).toBeUndefined();
			expect(() => storage.verificationShare(groups[0])).toThrowError();

			storage.registerVerification(groups[0], g(1n), g(2n));

			expect(storage.publicKey(groups[0])).toEqual(g(1n));
			expect(storage.verificationShare(groups[0])).toEqual(g(2n));
			expect(() => storage.registerVerification(groups[0], g(1n), g(2n))).toThrowError();
			expect(() => storage.registerVerification(groups[0], g(3n), g(4n))).toThrowError();
		});

		it("should register group signing shares", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.signingShare(groups[0])).toThrowError();
			expect(() => storage.registerSigningShare(groups[0], 42n)).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			expect(storage.signingShare(groups[0])).toBeUndefined();

			storage.registerSigningShare(groups[0], 42n);

			expect(storage.signingShare(groups[0])).toEqual(42n);
			expect(() => storage.registerSigningShare(groups[0], 42n)).toThrowError();
			expect(() => storage.registerSigningShare(groups[0], 1337n)).toThrowError();
		});

		it("should unregister groups and related data", () => {
			const storage = testStorage(participants[0].address);

			for (const groupId of [groups[0], groups[1]]) {
				storage.registerGroup(groupId, participants, 2n);
			}

			storage.unregisterGroup(groups[0]);

			expect(storage.knownGroups()).toEqual([groups[1]]);
			expect(() => storage.participantId(groups[0])).toThrowError();
			expect(() => storage.participants(groups[0])).toThrowError();
		});
	});

	describe("KeyGenInfoStorage", () => {
		const coefficients = [11n, 22n];
		const commitments = [
			{ id: 1n, value: [g(11n), g(22n)] },
			{ id: 2n, value: [g(33n), g(44n)] },
			{ id: 3n, value: [g(55n), g(66n)] },
		];
		const secretShares = [
			{ id: 1n, value: 101n },
			{ id: 2n, value: 102n },
			{ id: 3n, value: 103n },
		];

		it("should register KeyGen coefficients", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.registerKeyGen(groups[0], coefficients)).toThrowError();
			expect(() => storage.coefficients(groups[0])).toThrowError();
			expect(() => storage.encryptionKey(groups[0])).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			expect(() => storage.coefficients(groups[0])).toThrowError();
			expect(() => storage.encryptionKey(groups[0])).toThrowError();

			storage.registerKeyGen(groups[0], coefficients);

			expect(storage.coefficients(groups[0])).toEqual(coefficients);
			expect(storage.encryptionKey(groups[0])).toEqual(coefficients[0]);

			storage.clearKeyGen(groups[0]);

			expect(() => storage.coefficients(groups[0])).toThrowError();
			expect(() => storage.encryptionKey(groups[0])).toThrowError();
		});

		it("should register commitments", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.registerCommitments(groups[0], commitments[1].id, commitments[1].value)).toThrowError();
			expect(() => storage.commitments(groups[0], commitments[1].id)).toThrowError();
			expect(() => storage.missingCommitments(groups[0])).toThrowError();
			expect(() => storage.checkIfCommitmentsComplete(groups[0])).toThrowError();
			expect(() => storage.commitmentsMap(groups[0])).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			storage.registerCommitments(groups[0], commitments[1].id, commitments[1].value);

			expect(storage.missingCommitments(groups[0])).toEqual([commitments[0].id, commitments[2].id]);
			expect(storage.checkIfCommitmentsComplete(groups[0])).toBe(false);
			expect(() => storage.registerCommitments(groups[0], commitments[1].id, commitments[1].value)).toThrowError();

			storage.registerCommitments(groups[0], commitments[0].id, commitments[0].value);
			storage.registerCommitments(groups[0], commitments[2].id, commitments[2].value);

			expect(storage.missingCommitments(groups[0])).toEqual([]);
			expect(storage.checkIfCommitmentsComplete(groups[0])).toBe(true);

			for (const { id, value } of commitments) {
				expect(storage.commitments(groups[0], id)).toEqual(value);
			}

			expect(sortedEntries(storage.commitmentsMap(groups[0]))).toEqual(commitments.map((c) => [c.id, c.value]));

			storage.clearKeyGen(groups[0]);

			expect(storage.commitmentsMap(groups[0]).size).toBe(0);

			storage.unregisterGroup(groups[0]);

			expect(() => storage.missingCommitments(groups[0])).toThrowError();
			expect(() => storage.checkIfCommitmentsComplete(groups[0])).toThrowError();
			expect(() => storage.commitmentsMap(groups[0])).toThrowError();
		});

		it("should register secret shares", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.registerSecretShare(groups[0], secretShares[1].id, secretShares[1].value)).toThrowError();
			expect(() => storage.missingSecretShares(groups[0])).toThrowError();
			expect(() => storage.checkIfSecretSharesComplete(groups[0])).toThrowError();
			expect(() => storage.secretSharesMap(groups[0])).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			storage.registerSecretShare(groups[0], secretShares[1].id, secretShares[1].value);

			expect(storage.missingSecretShares(groups[0])).toEqual([secretShares[0].id, secretShares[2].id]);
			expect(storage.checkIfSecretSharesComplete(groups[0])).toBe(false);
			expect(() => storage.registerSecretShare(groups[0], secretShares[1].id, secretShares[1].value)).toThrowError();

			storage.registerSecretShare(groups[0], secretShares[0].id, secretShares[0].value);
			storage.registerSecretShare(groups[0], secretShares[2].id, secretShares[2].value);

			expect(storage.missingSecretShares(groups[0])).toEqual([]);
			expect(storage.checkIfSecretSharesComplete(groups[0])).toBe(true);

			expect(sortedEntries(storage.secretSharesMap(groups[0]))).toEqual(secretShares.map((s) => [s.id, s.value]));

			storage.clearKeyGen(groups[0]);

			expect(storage.secretSharesMap(groups[0]).size).toBe(0);

			storage.unregisterGroup(groups[0]);

			expect(() => storage.missingSecretShares(groups[0])).toThrowError();
			expect(() => storage.checkIfSecretSharesComplete(groups[0])).toThrowError();
			expect(() => storage.secretSharesMap(groups[0])).toThrowError();
		});
	});

	describe("NonceStorage", () => {
		const nonces = {
			root: `0x${"44".repeat(31)}01`,
			leaves: [`0x${"77".repeat(31)}01`, `0x${"77".repeat(31)}02`, `0x${"77".repeat(31)}03`, `0x${"77".repeat(31)}04`],
			commitments: [
				{
					hidingNonce: 401n,
					hidingNonceCommitment: g(401n),
					bindingNonce: 801n,
					bindingNonceCommitment: g(801n),
				},
				{
					hidingNonce: 402n,
					hidingNonceCommitment: g(402n),
					bindingNonce: 802n,
					bindingNonceCommitment: g(802n),
				},
				{
					hidingNonce: 403n,
					hidingNonceCommitment: g(403n),
					bindingNonce: 803n,
					bindingNonceCommitment: g(803n),
				},
				{
					hidingNonce: 404n,
					hidingNonceCommitment: g(404n),
					bindingNonce: 804n,
					bindingNonceCommitment: g(804n),
				},
			],
		} as const;
		const chunk = 42n;
		const offset = 2n;

		// TODO: The `NonceStorage` interface does not take a `readonly`
		// `NonceTree` as input, meaning that (at least at a type level) it
		// is permitted to modify values from the passed-in object. I don't
		// think we actually want this. For the test, we need to convert our
		// constant `readonly` value of `nonces` into a mutable one.
		const dup = (n: typeof nonces) => ({
			root: n.root,
			leaves: [...n.leaves],
			commitments: n.commitments.map((c) => ({ ...c })),
		});

		it("should register, link, and burn nonces", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.registerNonceTree(groups[0], dup(nonces))).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			expect(() => storage.linkNonceTree(groups[0], chunk, nonces.root)).toThrowError();
			expect(() => storage.nonceTree(groups[0], chunk)).toThrowError();
			expect(() => storage.burnNonce(groups[0], chunk, offset)).toThrowError();

			const treeHash = storage.registerNonceTree(groups[0], dup(nonces));

			expect(treeHash).toBe(nonces.root);
			expect(() => storage.nonceTree(groups[0], chunk)).toThrowError();
			expect(() => storage.burnNonce(groups[0], chunk, offset)).toThrowError();

			storage.linkNonceTree(groups[0], chunk, nonces.root);

			expect(storage.nonceTree(groups[0], chunk)).toEqual(nonces);

			storage.burnNonce(groups[0], chunk, offset);

			expect(storage.nonceTree(groups[0], chunk)).toEqual({
				...nonces,
				commitments: nonces.commitments.map((c, i) =>
					BigInt(i) === offset ? { ...c, hidingNonce: 0n, bindingNonce: 0n } : c,
				),
			});

			storage.unregisterGroup(groups[0]);

			expect(() => storage.nonceTree(groups[0], chunk)).toThrowError();
		});
	});

	describe("SignatureRequestStorage", () => {
		const signature = `0x${"55".repeat(31)}01` as const;
		const message = `0x${"deadc0de".repeat(8)}` as const;
		const signers = [participants[0].id, participants[2].id];
		const sequence = 1337n;

		const commitments = [
			{
				hidingNonceCommitment: g(5401n),
				bindingNonceCommitment: g(5801n),
			},
			{
				hidingNonceCommitment: g(5402n),
				bindingNonceCommitment: g(5802n),
			},
		];

		it("should register signature requests", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.registerSignatureRequest(signature, groups[0], message, signers, sequence)).toThrowError();
			expect(() => storage.signingGroup(signature)).toThrowError();
			expect(() => storage.signers(signature)).toThrowError();
			expect(() => storage.message(signature)).toThrowError();
			expect(() => storage.sequence(signature)).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			expect(() => storage.signingGroup(signature)).toThrowError();
			expect(() => storage.signers(signature)).toThrowError();
			expect(() => storage.message(signature)).toThrowError();
			expect(() => storage.sequence(signature)).toThrowError();

			storage.registerSignatureRequest(signature, groups[0], message, signers, sequence);

			expect(storage.signingGroup(signature)).toEqual(groups[0]);
			expect(storage.signers(signature)).toEqual(signers);
			expect(storage.message(signature)).toEqual(message);
			expect(storage.sequence(signature)).toEqual(sequence);

			storage.unregisterGroup(groups[0]);

			expect(() => storage.signingGroup(signature)).toThrowError();
			expect(() => storage.signers(signature)).toThrowError();
			expect(() => storage.message(signature)).toThrowError();
			expect(() => storage.sequence(signature)).toThrowError();
		});

		it("should register signature nonce commitments", () => {
			const storage = testStorage(participants[0].address);

			expect(() => storage.registerNonceCommitments(signature, signers[1], commitments[1])).toThrowError();
			expect(() => storage.checkIfNoncesComplete(signature)).toThrowError();
			expect(() => storage.missingNonces(signature)).toThrowError();
			expect(() => storage.nonceCommitmentsMap(signature)).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);
			storage.registerSignatureRequest(signature, groups[0], message, signers, sequence);

			expect(storage.checkIfNoncesComplete(signature)).toBe(false);
			expect(storage.missingNonces(signature)).toEqual(signers);
			expect(sortedEntries(storage.nonceCommitmentsMap(signature))).toEqual([]);

			storage.registerNonceCommitments(signature, signers[1], commitments[1]);

			expect(() => storage.registerNonceCommitments(signature, signers[1], commitments[1])).toThrowError();
			expect(storage.checkIfNoncesComplete(signature)).toBe(false);
			expect(storage.missingNonces(signature)).toEqual([signers[0]]);
			expect(sortedEntries(storage.nonceCommitmentsMap(signature))).toEqual([[signers[1], commitments[1]]]);

			storage.registerNonceCommitments(signature, signers[0], commitments[0]);

			expect(storage.checkIfNoncesComplete(signature)).toBe(true);
			expect(storage.missingNonces(signature)).toEqual([]);
			expect(sortedEntries(storage.nonceCommitmentsMap(signature))).toEqual([
				[signers[0], commitments[0]],
				[signers[1], commitments[1]],
			]);

			storage.unregisterGroup(groups[0]);

			expect(() => storage.checkIfNoncesComplete(signature)).toThrowError();
			expect(() => storage.missingNonces(signature)).toThrowError();
			expect(() => storage.nonceCommitmentsMap(signature)).toThrowError();
		});
	});
});
