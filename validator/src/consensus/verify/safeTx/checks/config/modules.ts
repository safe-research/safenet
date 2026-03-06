import { type Address, zeroAddress } from "viem";
import { buildSelectorCheck } from "../basic.js";
import { TransactionCheckError } from "../errors.js";

const ALLOWED_MODULES: Address[] = [
	"0x691f59471Bfd2B7d639DCF74671a2d648ED1E331", // AllowanceModule - 1.0.0
	"0x4Aa5Bf7D840aC607cb5BD3249e6Af6FC86C04897", // SocialRecoveryModule - 0.1.0
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
