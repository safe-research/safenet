import { resolve } from "node:path";
import { defineConfig } from "vitest/config";

export default defineConfig({
	define: {
		__BASE_PATH__: JSON.stringify("/"),
		__DOCS_URL__: JSON.stringify("https://docs.safefoundation.org/safenet"),
		__TERMS_URL__: JSON.stringify(""),
		__PRIVACY_URL__: JSON.stringify(""),
		__IMPRINT_URL__: JSON.stringify(""),
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
		},
	},
});
