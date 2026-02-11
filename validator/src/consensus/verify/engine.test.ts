import { describe, expect, it } from "vitest";
import { type PacketHandler, type Typed, VerificationEngine } from "./engine.js";

describe("verify engine", () => {
	it("should throw if no handler is present", async () => {
		const engine = new VerificationEngine(new Map());
		await expect(
			engine.verify({
				type: "test",
			}),
		).rejects.toStrictEqual(Error("No handler registered for type test"));
	});

	it("should throw if different handler is present", async () => {
		const handlers = new Map<string, PacketHandler<Typed>>();
		handlers.set("test", {
			hashAndVerify: async (_packet: Typed) => {
				throw new Error("not implemented");
			},
		});
		const engine = new VerificationEngine(handlers);
		await expect(
			engine.verify({
				type: "not_test",
			}),
		).rejects.toStrictEqual(Error("No handler registered for type not_test"));
	});

	it("should return valid verification result from handler on successful check", async () => {
		const handlers = new Map<string, PacketHandler<Typed>>();
		handlers.set("test", {
			hashAndVerify: async (_packet: Typed) => "0xbaddad42",
		});
		const engine = new VerificationEngine(handlers);
		await expect(
			engine.verify({
				type: "test",
			}),
		).resolves.toStrictEqual({
			status: "valid",
			packetId: "0xbaddad42",
		});
	});

	it("should return invalid verification result from handler on failed check", async () => {
		const handlers = new Map<string, PacketHandler<Typed>>();
		const error = new Error("Verification Error");
		handlers.set("test", {
			hashAndVerify: async (_packet: Typed) => {
				throw error;
			},
		});
		const engine = new VerificationEngine(handlers);
		await expect(
			engine.verify({
				type: "test",
			}),
		).resolves.toStrictEqual({
			status: "invalid",
			error,
		});
	});

	it("should return false if message is not verified", async () => {
		const engine = new VerificationEngine(new Map());
		expect(engine.isVerified("0xbaddad42")).toBeFalsy();
	});

	it("should return true if message is verified", async () => {
		const handlers = new Map<string, PacketHandler<Typed>>();
		handlers.set("test", {
			hashAndVerify: async (_packet: Typed) => "0xbaddad42",
		});
		const engine = new VerificationEngine(handlers);
		await engine.verify({ type: "test" });
		expect(engine.isVerified("0xbaddad42")).toBeTruthy();
	});
});
