import { type Address, zeroAddress } from "viem";
import { buildSelectorCheck } from "../basic.js";
import { classifyTxCheck } from "../errors.js";

const ALLOWED_MODULES: Address[] = [
	// No modules allowed right now!
];

export const buildEnableModuleCheck = () =>
	classifyTxCheck(
		"unknown_module",
		buildSelectorCheck("function enableModule(address)", ([module]) => {
			if (!ALLOWED_MODULES.includes(module)) {
				throw Error(`Cannot enable unknown module ${module}`);
			}
		}),
	);

const ALLOWED_MODULE_GUARDS: Address[] = [
	// No module guards allowed right now!
];

export const buildSetModuleGuardCheck = () =>
	classifyTxCheck(
		"unknown_module_guard",
		buildSelectorCheck("function setModuleGuard(address)", ([guard]) => {
			if (guard !== zeroAddress && !ALLOWED_MODULE_GUARDS.includes(guard)) {
				throw Error(`Cannot set unknown module guard ${guard}`);
			}
		}),
	);
