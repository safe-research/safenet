import type { Prettify } from "viem";

type MergeDefaults<T extends object, D extends object> = Prettify<{
	[K in keyof T | keyof D]: K extends keyof T
		? undefined extends T[K]
			? // If T[K] can be undefined, remove undefined and union with Default
				Exclude<T[K], undefined> | (K extends keyof D ? D[K] : never)
			: // If T[K] cannot be undefined, use T[K] strictly
				T[K]
		: K extends keyof D
			? D[K]
			: never; // If only in Default, use Default
}>;

export const withDefaults = <T extends Record<string, unknown>, D extends Record<string, unknown>>(
	config: T,
	defaultValues: D,
): MergeDefaults<T, D> => {
	const merged: Record<string, unknown> = { ...defaultValues };
	for (const key in config) {
		const value = config[key];
		if (value !== undefined) {
			merged[key] = value;
		}
	}
	return merged as MergeDefaults<T, D>;
};
