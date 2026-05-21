import type { Address } from "viem";
import { z } from "zod";
import { checkedAddressSchema } from "@/lib/schemas";

const STORAGE_KEY_SETTINGS = "localStorage.settings.object.v1";
const STORAGE_KEY_SAFE_API_SETTINGS = "localStorage.safe_api_settings.object.v1";
const STORAGE_KEY_SAFE_UI_SETTINGS = "localStorage.safe_ui_settings.object.v1";

const DEFAULT_SETTINGS = {
	consensus: __DEFAULT_CONSENSUS__ as Address,
	rpc: __DEFAULT_RPC__,
	decoder: __DEFAULT_DECODER__,
	maxBlockRange: __DEFAULT_MAX_BLOCK_RANGE__,
	validatorInfo: __DEFAULT_VALIDATOR_INFO__,
	refetchInterval: __DEFAULT_REFETCH_INTERVAL__,
	blocksPerEpoch: __DEFAULT_BLOCKS_PER_EPOCH__,
	signingTimeout: __DEFAULT_SIGNING_TIMEOUT__,
	relayer: __DEFAULT_RELAYER__ || undefined,
};

const settingsSchema = z.object({
	rpc: z.url().default(DEFAULT_SETTINGS.rpc),
	decoder: z.url().default(DEFAULT_SETTINGS.decoder),
	consensus: checkedAddressSchema.default(DEFAULT_SETTINGS.consensus),
	relayer: z.url().optional(),
	maxBlockRange: z.number().int().positive().default(DEFAULT_SETTINGS.maxBlockRange),
	validatorInfo: z.url().default(DEFAULT_SETTINGS.validatorInfo),
	refetchInterval: z.number().int().nonnegative().default(DEFAULT_SETTINGS.refetchInterval),
	blocksPerEpoch: z.number().int().positive().default(DEFAULT_SETTINGS.blocksPerEpoch),
	signingTimeout: z.number().int().positive().default(DEFAULT_SETTINGS.signingTimeout),
});

export type Settings = z.output<typeof settingsSchema>;

export function loadSettings(): Settings {
	try {
		const stored = localStorage.getItem(STORAGE_KEY_SETTINGS);
		const parsed = stored ? JSON.parse(stored) : {};
		return settingsSchema.parse({ ...DEFAULT_SETTINGS, ...parsed });
	} catch (e) {
		console.error(e);
		return settingsSchema.parse(DEFAULT_SETTINGS);
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
	url: z.url().default(DEFAULT_API_SETTINGS.url),
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

const DEFAULT_UI_SETTINGS = {};

const uiSettingsSchema = z.object({
	theme: z.union([z.literal("dark"), z.literal("light")]).optional(),
});

export type UiSettings = z.output<typeof uiSettingsSchema>;

export function loadUiSettings(): UiSettings {
	try {
		const stored = localStorage.getItem(STORAGE_KEY_SAFE_UI_SETTINGS);
		return stored ? uiSettingsSchema.parse(JSON.parse(stored)) : DEFAULT_UI_SETTINGS;
	} catch (e) {
		console.error(e);
		return DEFAULT_UI_SETTINGS;
	}
}

export function updateUiSettings(settings: Partial<UiSettings>) {
	localStorage.setItem(STORAGE_KEY_SAFE_UI_SETTINGS, JSON.stringify(settings));
	if (
		settings.theme === "dark" ||
		(settings.theme === undefined && window.matchMedia("(prefers-color-scheme: dark)").matches)
	) {
		document.documentElement.classList.add("dark");
	} else {
		document.documentElement.classList.remove("dark");
	}
}
