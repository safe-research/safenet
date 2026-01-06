import { toFunctionSelector } from "viem";
import type { TransactionCheck } from "../../handler.js";
import { buildFixedParamsCheck, buildSupportedSelectorCheck } from "../basic.js";
import { buildCombinedChecks } from "../combined.js";

const buildMigrationCheck = () =>
	buildCombinedChecks([
		buildFixedParamsCheck({ operation: 1 }),
		buildSupportedSelectorCheck(
			[
				toFunctionSelector("function migrateSingleton()"),
				toFunctionSelector("function migrateWithFallbackHandler()"),
				toFunctionSelector("function migrateL2Singleton()"),
				toFunctionSelector("function migrateL2WithFallbackHandler()"),
			],
			false,
		),
	]);

export const buildSingletonUpgradeChecks = (): Record<string, TransactionCheck> => {
	const migrationCheck = buildMigrationCheck();
	return {
		"0x6439e7ABD8Bb915A5263094784C5CF561c4172AC": migrationCheck,
		"0x526643F69b81B008F46d95CD5ced5eC0edFFDaC6": migrationCheck,
	};
};
