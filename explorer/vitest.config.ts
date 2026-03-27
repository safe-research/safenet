import { resolve } from "node:path";
import { defineConfig } from "vitest/config";

export default defineConfig({
	define: {
		__BASE_PATH__: JSON.stringify("/"),
		__DOCS_URL__: JSON.stringify("https://docs.safefoundation.org/safenet"),
		__TERMS_URL__: JSON.stringify("https://test.example/terms"),
		__PRIVACY_URL__: JSON.stringify("https://test.example/privacy"),
		__IMPRINT_URL__: JSON.stringify("https://test.example/imprint"),
		__DEFAULT_CONSENSUS__: JSON.stringify("0x223624cBF099e5a8f8cD5aF22aFa424a1d1acEE9"),
		__DEFAULT_RPC__: JSON.stringify("https://1rpc.io/gnosis"),
		__DEFAULT_DECODER__: JSON.stringify("https://calldata.swiss-knife.xyz/decoder?calldata="),
		__DEFAULT_RELAYER__: JSON.stringify(""),
		__DEFAULT_MAX_BLOCK_RANGE__: 10000,
		__DEFAULT_VALIDATOR_INFO__: JSON.stringify(
			"https://raw.githubusercontent.com/safe-fndn/safenet-beta-data/refs/heads/main/assets/validator-info.json",
		),
		__DEFAULT_REFETCH_INTERVAL__: 10000,
		__DEFAULT_BLOCKS_PER_EPOCH__: 1440,
		__DEFAULT_SIGNING_TIMEOUT__: 12,
	},
	test: {
		coverage: {
			provider: "v8",
			include: ["src/**/*.ts", "src/**/*.tsx"],
			exclude: ["src/__tests__/**"],
			reporter: ["text", "json", "html", "lcov"],
		},
	},
	resolve: {
		alias: {
			"@": resolve(__dirname, "./src"),
			"@plausible-analytics/tracker": resolve(__dirname, "../node_modules/@plausible-analytics/tracker/plausible.js"),
		},
	},
});
