import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
	// Load environment variables and set base path for nested routes
	const env = loadEnv(mode, process.cwd());

	// Normalize base path to ensure it always starts and ends with a slash.
	// Relative paths (e.g. "./") are passed through unchanged — they are used for
	// IPFS path-gateway deployments where absolute paths would resolve to the
	// gateway domain root instead of the CID subtree.
	let basePath = env.VITE_BASE_PATH || "/";
	if (!basePath.startsWith(".")) {
		if (!basePath.startsWith("/")) {
			basePath = `/${basePath}`;
		}
		if (!basePath.endsWith("/")) {
			basePath = `${basePath}/`;
		}
	}

	// Validate VITE_DEFAULT_* overrides at build time (only when explicitly set)
	if (env.VITE_DEFAULT_CONSENSUS && !/^0x[0-9a-fA-F]{40}$/.test(env.VITE_DEFAULT_CONSENSUS)) {
		throw new Error(`VITE_DEFAULT_CONSENSUS is not a valid Ethereum address: ${env.VITE_DEFAULT_CONSENSUS}`);
	}
	for (const key of [
		"VITE_DEFAULT_RPC",
		"VITE_DEFAULT_DECODER",
		"VITE_DEFAULT_RELAYER",
		"VITE_DEFAULT_VALIDATOR_INFO",
	]) {
		if (env[key]) {
			try {
				new URL(env[key]);
			} catch {
				throw new Error(`${key} is not a valid URL: ${env[key]}`);
			}
		}
	}
	for (const key of [
		"VITE_DEFAULT_MAX_BLOCK_RANGE",
		"VITE_DEFAULT_REFETCH_INTERVAL",
		"VITE_DEFAULT_BLOCKS_PER_EPOCH",
		"VITE_DEFAULT_SIGNING_TIMEOUT",
	]) {
		if (env[key] && !Number.isInteger(Number(env[key]))) {
			throw new Error(`${key} is not a valid integer: ${env[key]}`);
		}
	}

	return {
		base: basePath,
		worker: {
			format: "es",
		},
		plugins: [
			tanstackRouter({
				target: "react",
				autoCodeSplitting: true,
				routeFileIgnorePattern: ".test.tsx?",
			}),
			viteReact(),
			tailwindcss(),
		],
		test: {
			globals: true,
			environment: "jsdom",
		},
		resolve: {
			alias: {
				"@": resolve(__dirname, "./src"),
			},
		},
		define: {
			// Expose the normalized base path as a constant that can be used in client code
			__BASE_PATH__: JSON.stringify(basePath),
			// Link URLs — configurable per deployment, with sensible defaults
			__DOCS_URL__: JSON.stringify(env.VITE_DOCS_URL || "https://docs.safefoundation.org/safenet"),
			__TERMS_URL__: JSON.stringify(env.VITE_TERMS_URL || "#tos"),
			__PRIVACY_URL__: JSON.stringify(env.VITE_PRIVACY_URL || "#privacy"),
			__IMPRINT_URL__: JSON.stringify(env.VITE_IMPRINT_URL || "#imprint"),
			// Default explorer settings — configurable per deployment, users can still override in the UI
			__DEFAULT_CONSENSUS__: JSON.stringify(env.VITE_DEFAULT_CONSENSUS || "0x223624cBF099e5a8f8cD5aF22aFa424a1d1acEE9"),
			__DEFAULT_RPC__: JSON.stringify(env.VITE_DEFAULT_RPC || "https://1rpc.io/gnosis"),
			__DEFAULT_DECODER__: JSON.stringify(
				env.VITE_DEFAULT_DECODER || "https://calldata.swiss-knife.xyz/decoder?calldata=",
			),
			__DEFAULT_RELAYER__: JSON.stringify(env.VITE_DEFAULT_RELAYER || ""),
			__DEFAULT_MAX_BLOCK_RANGE__: Number(env.VITE_DEFAULT_MAX_BLOCK_RANGE) || 10000,
			__DEFAULT_VALIDATOR_INFO__: JSON.stringify(
				env.VITE_DEFAULT_VALIDATOR_INFO ||
					"https://raw.githubusercontent.com/safe-fndn/safenet-beta-data/refs/heads/main/assets/validator-info.json",
			),
			__DEFAULT_REFETCH_INTERVAL__: Number(env.VITE_DEFAULT_REFETCH_INTERVAL) || 10000,
			__DEFAULT_BLOCKS_PER_EPOCH__: Number(env.VITE_DEFAULT_BLOCKS_PER_EPOCH) || 1440,
			__DEFAULT_SIGNING_TIMEOUT__: Number(env.VITE_DEFAULT_SIGNING_TIMEOUT) || 12,
		},
	};
});
