import { useCallback, useEffect, useState } from "react";
import { loadSettings, loadUiSettings, type Settings, type UiSettings } from "@/lib/settings";

export function useSettings(): [Settings, () => void] {
	const [currentSettings, setCurrentSettings] = useState(loadSettings());
	const load = useCallback(async () => {
		setCurrentSettings(loadSettings());
	}, []);
	useEffect(() => {
		load();
	}, [load]);
	return [
		currentSettings,
		() => {
			load();
		},
	];
}

export function useUiSettings(): [UiSettings, () => void] {
	const [currentSettings, setCurrentSettings] = useState(loadUiSettings());
	const load = useCallback(async () => {
		setCurrentSettings(loadUiSettings());
	}, []);
	useEffect(() => {
		load();
	}, [load]);
	return [
		currentSettings,
		() => {
			load();
		},
	];
}
