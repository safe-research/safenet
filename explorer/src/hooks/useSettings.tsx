import { loadSettings, loadUiSettings, type Settings, type UiSettings } from "@/lib/settings";

export function useSettings(): [Settings] {
	return [loadSettings()];
}

export function useUiSettings(): [UiSettings] {
	return [loadUiSettings()];
}
