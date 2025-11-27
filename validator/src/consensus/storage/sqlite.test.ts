import { describe, expect, it } from "vitest";
import { g } from "../../frost/math.js";
import { SqliteStorage } from "./sqlite.js";

const groups = [`0x${"ff".repeat(31)}01`, `0x${"ff".repeat(31)}02`] as const;

const participants = [
	{ id: 1n, address: "0x0000000000000000000000000000000000000001" },
	{ id: 2n, address: "0x0000000000000000000000000000000000000002" },
	{ id: 3n, address: "0x0000000000000000000000000000000000000003" },
] as const;

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
});
