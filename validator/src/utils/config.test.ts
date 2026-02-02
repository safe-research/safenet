import { describe, expect, it } from "vitest";
import { withDefaults } from "./config.js";

describe("withDefaults", () => {
	it("should not overwrite defaults with undefined", async () => {
		const defaultConfig = { first: "123", second: undefined };
		const config = { first: undefined, second: 123 };

		const merged = withDefaults(config, defaultConfig);
		expect(merged).toStrictEqual({ first: "123", second: 123 });
	});

	it("should use all default values for empty config", async () => {
		const defaultConfig = { first: "123", second: 123 };
		const merged = withDefaults({}, defaultConfig);
		expect(merged).toStrictEqual({ first: "123", second: 123 });
	});

	it("should merge with no overwrites", async () => {
		const defaultConfig = { first: "123" };
		const config = { second: 123 };
		const merged = withDefaults(config, defaultConfig);
		expect(merged).toStrictEqual({ first: "123", second: 123 });
	});

	it("should keep empty default value", async () => {
		const defaultConfig = { first: undefined };
		const config = { second: 123 };
		const merged = withDefaults(config, defaultConfig);
		expect(merged).toStrictEqual({ first: undefined, second: 123 });
	});

	it("should drop empty config value", async () => {
		const defaultConfig = { second: 123 };
		const config = { first: undefined };
		const merged = withDefaults(config, defaultConfig);
		expect(merged).toStrictEqual({ second: 123 });
	});

	it("should merge complex types", async () => {
		const defaultConfig = {
			blockPropagationDelay: 500,
			blockRetryDelays: [200, 100, 100],
			timer: {
				now: Date.now,
				sleep(ms: number): Promise<void> {
					return new Promise((resolve) => setTimeout(resolve, ms));
				},
			},
		};
		const config = {
			blockTime: 12000,
			maxReorgDepth: 5,
			blockPropagationDelay: undefined,
			blockRetryDelays: undefined,
			timer: undefined,
		};
		const merged = withDefaults(config, defaultConfig);
		expect(merged).toStrictEqual({
			blockTime: 12000,
			maxReorgDepth: 5,
			blockPropagationDelay: 500,
			blockRetryDelays: defaultConfig.blockRetryDelays,
			timer: defaultConfig.timer,
		});
	});
});
