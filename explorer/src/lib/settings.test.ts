// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadSettings } from "./settings";

const STORAGE_KEY = "localStorage.settings.object.v1";

describe("loadSettings", () => {
	beforeEach(() => {
		localStorage.clear();
	});

	afterEach(() => {
		localStorage.clear();
	});

	it("signingTimeout defaults to 12 when nothing is stored", () => {
		const settings = loadSettings();
		expect(settings.signingTimeout).toBe(12);
	});

	it("signingTimeout is read from stored settings", () => {
		localStorage.setItem(STORAGE_KEY, JSON.stringify({ signingTimeout: 24 }));
		const settings = loadSettings();
		expect(settings.signingTimeout).toBe(24);
	});

	it("relayer defaults to undefined when nothing is stored", () => {
		const settings = loadSettings();
		expect(settings.relayer).toBeUndefined();
	});

	it("relayer is read from stored settings", () => {
		localStorage.setItem(STORAGE_KEY, JSON.stringify({ relayer: "https://relayer.example.com" }));
		const settings = loadSettings();
		expect(settings.relayer).toBe("https://relayer.example.com");
	});

	it("stored settings override defaults but unset fields fall back to defaults", () => {
		localStorage.setItem(STORAGE_KEY, JSON.stringify({ signingTimeout: 99 }));
		const settings = loadSettings();
		expect(settings.signingTimeout).toBe(99);
		expect(settings.blocksPerEpoch).toBe(1440);
		expect(settings.relayer).toBeUndefined();
	});

	it("returns default settings when stored data is malformed", () => {
		localStorage.setItem(STORAGE_KEY, "not valid json{");
		const settings = loadSettings();
		expect(settings.signingTimeout).toBe(12);
		expect(settings.blocksPerEpoch).toBe(1440);
	});

	it("oracles defaults to an empty array when nothing is stored", () => {
		const settings = loadSettings();
		expect(settings.oracles).toEqual([]);
	});

	it("oracles is read from stored settings", () => {
		const oracle = "0x49Db717Adec0D22235A73C3a9c2ea57AB0bC2353";
		localStorage.setItem(STORAGE_KEY, JSON.stringify({ oracles: [oracle] }));
		const settings = loadSettings();
		expect(settings.oracles).toEqual([oracle]);
	});

	it("falls back to defaults when a stored oracle address is invalid", () => {
		localStorage.setItem(STORAGE_KEY, JSON.stringify({ oracles: ["not-an-address"] }));
		const settings = loadSettings();
		expect(settings.oracles).toEqual([]);
	});
});
