import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		coverage: {
			provider: 'v8',
			include: ['src/**/*.ts', 'src/**/*.tsx'],
			exclude: ['src/__tests__/**'],
			reporter: ['text', 'json', 'html', 'lcov'],
		},
	},
});
