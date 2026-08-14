import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

const ETHEREUM_ADDRESS_REGEX = /^0x[0-9a-fA-F]{40}$/;

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
	// Load environment variables and set base path for nested routes
	const env = loadEnv(mode, process.cwd());

	// Default to "" (Vite's relative base) so asset URLs resolve correctly at any
	// mount point — IPFS path gateways, subdomain gateways, and regular web
	// servers at any subpath — without knowing the deployment path at build time.
	// Set VITE_BASE_PATH to an explicit absolute path (e.g. /safenet/) only when
	// assets must be served from a different origin than the HTML (e.g. a CDN).
	let basePath = env.VITE_BASE_PATH || "";
	if (basePath) {
		if (!basePath.startsWith("/")) {
			basePath = `/${basePath}`;
		}
		if (!basePath.endsWith("/")) {
			basePath = `${basePath}/`;
		}
	}

	// Validate VITE_DEFAULT_* overrides at build time (only when explicitly set)
	if (env.VITE_DEFAULT_CONSENSUS && !ETHEREUM_ADDRESS_REGEX.test(env.VITE_DEFAULT_CONSENSUS)) {
		throw new Error(`VITE_DEFAULT_CONSENSUS is not a valid Ethereum address: ${env.VITE_DEFAULT_CONSENSUS}`);
	}
	for (const key of [
		"VITE_DEFAULT_RPC",
		"VITE_DEFAULT_DECODER",
		"VITE_DEFAULT_RELAYER",
		"VITE_DEFAULT_VALIDATOR_INFO",
		"VITE_DEFAULT_SENTINEL_INFO",
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
		"VITE_DEFAULT_DETAILS_MAX_BLOCK_RANGE",
		"VITE_DEFAULT_REFETCH_INTERVAL",
		"VITE_DEFAULT_BLOCKS_PER_EPOCH",
		"VITE_DEFAULT_SIGNING_TIMEOUT",
	]) {
		if (env[key] && !Number.isInteger(Number(env[key]))) {
			throw new Error(`${key} is not a valid integer: ${env[key]}`);
		}
	}
	const defaultOracles = (env.VITE_DEFAULT_ORACLES || "")
		.split(",")
		.map((address) => address.trim())
		.filter(Boolean);
	for (const address of defaultOracles) {
		if (!ETHEREUM_ADDRESS_REGEX.test(address)) {
			throw new Error(`VITE_DEFAULT_ORACLES contains an invalid Ethereum address: ${address}`);
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
			{
				// %VITE_APP_URL% is not substituted by Vite when the var is unset;
				// this ensures it always resolves (empty string = tag ignored by crawlers).
				// Falls back to CF_PAGES_URL so Cloudflare Pages preview deployments work automatically.
				name: "inject-app-url",
				transformIndexHtml: (html) =>
					html.replace(/%VITE_APP_URL%/g, env.VITE_APP_URL || process.env.CF_PAGES_URL || ""),
			},
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
			__DEFAULT_RPC__: JSON.stringify(env.VITE_DEFAULT_RPC || "https://rpc.gnosischain.com/"),
			__DEFAULT_DECODER__: JSON.stringify(
				env.VITE_DEFAULT_DECODER || "https://calldata.swiss-knife.xyz/decoder?calldata=",
			),
			__DEFAULT_RELAYER__: JSON.stringify(env.VITE_DEFAULT_RELAYER || ""),
			__DEFAULT_MAX_BLOCK_RANGE__: Number(env.VITE_DEFAULT_MAX_BLOCK_RANGE) || 10000,
			// 0 means unlimited (no fromBlock lower bound) — used for single-proposal detail
			// lookups, which need to find proposals arbitrarily far in the past.
			__DEFAULT_DETAILS_MAX_BLOCK_RANGE__: Number(env.VITE_DEFAULT_DETAILS_MAX_BLOCK_RANGE) || 0,
			__DEFAULT_VALIDATOR_INFO__: JSON.stringify(
				env.VITE_DEFAULT_VALIDATOR_INFO ||
					"https://raw.githubusercontent.com/safe-fndn/safenet-beta-data/refs/heads/main/assets/validator-info.json",
			),
			__DEFAULT_SENTINEL_INFO__: JSON.stringify(
				env.VITE_DEFAULT_SENTINEL_INFO ||
					"https://raw.githubusercontent.com/safe-fndn/safenet-beta-data/refs/heads/main/assets/sentinel-info.json",
			),
			__DEFAULT_REFETCH_INTERVAL__: Number(env.VITE_DEFAULT_REFETCH_INTERVAL) || 10000,
			__DEFAULT_BLOCKS_PER_EPOCH__: Number(env.VITE_DEFAULT_BLOCKS_PER_EPOCH) || 1440,
			__DEFAULT_SIGNING_TIMEOUT__: Number(env.VITE_DEFAULT_SIGNING_TIMEOUT) || 12,
			__DEFAULT_ORACLES__: JSON.stringify(defaultOracles),
		},
	};
});
