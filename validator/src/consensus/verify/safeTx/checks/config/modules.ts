import { type Address, zeroAddress } from "viem";
import { buildSelectorCheck } from "../basic.js";
import { TransactionCheckError } from "../errors.js";

const ALLOWED_MODULES: Address[] = [
	// No modules allowed right now!
];

export const buildEnableModuleCheck = () =>
	buildSelectorCheck("function enableModule(address)", ([module]) => {
		if (!ALLOWED_MODULES.includes(module)) {
			throw new TransactionCheckError("unsupported_module", `Cannot enable unknown module ${module}`);
		}
	});

const ALLOWED_MODULE_GUARDS: Address[] = [
	// No module guards allowed right now!
];

export const buildSetModuleGuardCheck = () =>
	buildSelectorCheck("function setModuleGuard(address)", ([guard]) => {
		if (guard !== zeroAddress && !ALLOWED_MODULE_GUARDS.includes(guard)) {
			throw new TransactionCheckError("unsupported_module_guard", `Cannot set unknown module guard ${guard}`);
		}
	});
