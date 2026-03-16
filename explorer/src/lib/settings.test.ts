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
});
