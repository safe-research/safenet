import { describe, expect, it } from "vitest";
import { g } from "../../frost/math.js";
import { SqliteStorage } from "./sqlite.js";

const groups = [`0x${"ff".repeat(31)}01`, `0x${"ff".repeat(31)}02`] as const;

const participants = [
	{ id: 1n, address: "0x0000000000000000000000000000000000000001" },
	{ id: 2n, address: "0x0000000000000000000000000000000000000002" },
	{ id: 3n, address: "0x0000000000000000000000000000000000000003" },
] as const;

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

const sortedEntries = <T>(m: Map<bigint, T>): [bigint, T][] => {
	return [...m.entries()].sort(([a], [b]) => Number(a - b));
};

describe("sqlite", () => {
	describe("GroupInfoStorage", () => {
		it("should register groups with participants", () => {
			const storage = new SqliteStorage(participants[1].address, ":memory:");

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
			const storage = new SqliteStorage(participants[0].address, ":memory:");

			expect(() => storage.publicKey(groups[0])).toThrowError();
			expect(() => storage.verificationShare(groups[0])).toThrowError();
			expect(() =>
				storage.registerVerification(groups[0], g(1n), g(2n)),
			).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			expect(storage.publicKey(groups[0])).toBeUndefined();
			expect(() => storage.verificationShare(groups[0])).toThrowError();

			storage.registerVerification(groups[0], g(1n), g(2n));

			expect(storage.publicKey(groups[0])).toEqual(g(1n));
			expect(storage.verificationShare(groups[0])).toEqual(g(2n));
			expect(() =>
				storage.registerVerification(groups[0], g(1n), g(2n)),
			).toThrowError();
			expect(() =>
				storage.registerVerification(groups[0], g(3n), g(4n)),
			).toThrowError();
		});

		it("should register group signing shares", () => {
			const storage = new SqliteStorage(participants[0].address, ":memory:");

			expect(() => storage.signingShare(groups[0])).toThrowError();
			expect(() => storage.registerSigningShare(groups[0], 42n)).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			expect(storage.signingShare(groups[0])).toBeUndefined();

			storage.registerSigningShare(groups[0], 42n);

			expect(storage.signingShare(groups[0])).toEqual(42n);
			expect(() => storage.registerSigningShare(groups[0], 42n)).toThrowError();
			expect(() =>
				storage.registerSigningShare(groups[0], 1337n),
			).toThrowError();
		});

		it("should unregister groups and related data", () => {
			const storage = new SqliteStorage(participants[0].address, ":memory:");

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
		it("should register KeyGen coefficients", () => {
			const storage = new SqliteStorage(participants[0].address, ":memory:");

			expect(() =>
				storage.registerKeyGen(groups[0], coefficients),
			).toThrowError();
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
			const storage = new SqliteStorage(participants[0].address, ":memory:");

			expect(() =>
				storage.registerCommitments(
					groups[0],
					commitments[1].id,
					commitments[1].value,
				),
			).toThrowError();
			expect(() =>
				storage.commitments(groups[0], commitments[1].id),
			).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			storage.registerCommitments(
				groups[0],
				commitments[1].id,
				commitments[1].value,
			);

			expect(storage.missingCommitments(groups[0])).toEqual([
				commitments[0].id,
				commitments[2].id,
			]);
			expect(storage.checkIfCommitmentsComplete(groups[0])).toBe(false);
			expect(() =>
				storage.registerCommitments(
					groups[0],
					commitments[1].id,
					commitments[1].value,
				),
			).toThrowError();

			storage.registerCommitments(
				groups[0],
				commitments[0].id,
				commitments[0].value,
			);
			storage.registerCommitments(
				groups[0],
				commitments[2].id,
				commitments[2].value,
			);

			expect(storage.missingCommitments(groups[0])).toEqual([]);
			expect(storage.checkIfCommitmentsComplete(groups[0])).toBe(true);

			for (const { id, value } of commitments) {
				expect(storage.commitments(groups[0], id)).toEqual(value);
			}

			expect(sortedEntries(storage.commitmentsMap(groups[0]))).toEqual(
				commitments.map((c) => [c.id, c.value]),
			);

			storage.clearKeyGen(groups[0]);

			expect(storage.commitmentsMap(groups[0]).size).toBe(0);
		});

		it("should register secret shares", () => {
			const storage = new SqliteStorage(participants[0].address, ":memory:");

			expect(() =>
				storage.registerSecretShare(
					groups[0],
					secretShares[1].id,
					secretShares[1].value,
				),
			).toThrowError();

			storage.registerGroup(groups[0], participants, 2n);

			storage.registerSecretShare(
				groups[0],
				secretShares[1].id,
				secretShares[1].value,
			);

			expect(storage.missingSecretShares(groups[0])).toEqual([
				secretShares[0].id,
				secretShares[2].id,
			]);
			expect(storage.checkIfSecretSharesComplete(groups[0])).toBe(false);
			expect(() =>
				storage.registerSecretShare(
					groups[0],
					secretShares[1].id,
					secretShares[1].value,
				),
			).toThrowError();

			storage.registerSecretShare(
				groups[0],
				secretShares[0].id,
				secretShares[0].value,
			);
			storage.registerSecretShare(
				groups[0],
				secretShares[2].id,
				secretShares[2].value,
			);

			expect(storage.missingSecretShares(groups[0])).toEqual([]);
			expect(storage.checkIfSecretSharesComplete(groups[0])).toBe(true);

			expect(sortedEntries(storage.secretSharesMap(groups[0]))).toEqual(
				secretShares.map((s) => [s.id, s.value]),
			);

			storage.clearKeyGen(groups[0]);

			expect(storage.secretSharesMap(groups[0]).size).toBe(0);
		});
	});
});
