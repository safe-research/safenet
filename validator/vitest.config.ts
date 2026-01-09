import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		/**
		 * This is the most important setting for a Node.js project.
		 * It ensures that Vitest uses a pure Node.js environment
		 * without any browser/JSDOM APIs (like `window` or `document`).
		 */
		environment: "node",

		/**
		 * This is the default, but it's good to be explicit.
		 * It matches your test file's style, where you
		 * `import { describe, it, expect } from 'vitest'`.
		 * Set to `true` if you want test globals available everywhere.
		 */
		globals: false,

		coverage: {
			provider: 'v8',
			exclude: ['src/__tests__/**'],
			reporter: ['text', 'json', 'html', 'lcov'],
		},
	},
});
