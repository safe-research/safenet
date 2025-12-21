import type { Address } from "viem";
import { z } from "zod";
import { checkedAddressSchema } from "./schemas";

const STORAGE_KEY_SETTINGS = "localStorage.settings.object.v1";
const STORAGE_KEY_SAFE_API_SETTINGS = "localStorage.safe_api_settings.object.v1";

const DEFAULT_SETTINGS = {
	consensus: "0xF39F38a7e40fD51C7c5f355d92A0AFA75776871F" as Address,
	rpc: "https://ethereum-sepolia-rpc.publicnode.com",
	decoder: "https://calldata.swiss-knife.xyz/decoder?calldata=",
};

const settingsSchema = z.object({
	rpc: z.url().default(DEFAULT_SETTINGS.rpc),
	decoder: z.url().default(DEFAULT_SETTINGS.decoder),
	consensus: checkedAddressSchema.default(DEFAULT_SETTINGS.consensus),
	relayer: z.url().optional(),
});

export type Settings = z.output<typeof settingsSchema>;

export function loadSettings(): Settings {
	try {
		const stored = localStorage.getItem(STORAGE_KEY_SETTINGS);
		return stored ? settingsSchema.parse(JSON.parse(stored)) : DEFAULT_SETTINGS;
	} catch (e) {
		console.error(e);
		return DEFAULT_SETTINGS;
	}
}

export function updateSettings(settings: Partial<Settings>) {
	localStorage.setItem(STORAGE_KEY_SETTINGS, JSON.stringify(settings));
}

const DEFAULT_API_SETTINGS = {
	url: "https://api.safe.global",
};

const safeApiSettingsSchema = z.object({
	apiKey: z.string().optional(),
	url: z.url().default(DEFAULT_SETTINGS.decoder),
});

export type SafeApiSettings = z.output<typeof safeApiSettingsSchema>;

export function loadSafeApiSettings(): SafeApiSettings {
	try {
		const stored = localStorage.getItem(STORAGE_KEY_SAFE_API_SETTINGS);
		return stored ? safeApiSettingsSchema.parse(JSON.parse(stored)) : DEFAULT_API_SETTINGS;
	} catch (e) {
		console.error(e);
		return DEFAULT_API_SETTINGS;
	}
}

export function updateSafeApiSettings(settings: Partial<SafeApiSettings>) {
	localStorage.setItem(STORAGE_KEY_SAFE_API_SETTINGS, JSON.stringify(settings));
}
