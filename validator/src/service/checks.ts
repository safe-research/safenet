import {
	buildNoDelegateCallCheck,
	buildSelectorChecks,
	buildSelfCheck,
	buildSupportedSignaturesCheck,
} from "../consensus/verify/safeTx/checks/basic.js";
import { buildAddressSplitCheck, buildCombinedChecks } from "../consensus/verify/safeTx/checks/combined.js";
import { buildSetFallbackHandlerCheck } from "../consensus/verify/safeTx/checks/config/fallback.js";
import { buildSetGuardCheck } from "../consensus/verify/safeTx/checks/config/guards.js";
import { buildSignMessageChecks } from "../consensus/verify/safeTx/checks/config/messages.js";
import { buildEnableModuleCheck, buildSetModuleGuardCheck } from "../consensus/verify/safeTx/checks/config/modules.js";
import { buildSingletonUpgradeChecks } from "../consensus/verify/safeTx/checks/config/singletons.js";
import { buildMultiSendCallOnlyCheck } from "../consensus/verify/safeTx/checks/multisend.js";
import type { TransactionCheck } from "../consensus/verify/safeTx/handler.js";

export const buildSafeTransactionCheck = (): TransactionCheck => {
	// Only specific calls should be allowed on the Safe itself
	const selfChecks = buildSelfCheck(
		buildSelectorChecks(
			"invalid_self_call",
			// These methods have additional validation on their arguments.
			{
				...buildSetFallbackHandlerCheck(),
				...buildSetGuardCheck(),
				...buildSetModuleGuardCheck(),
				...buildEnableModuleCheck(),
			},
			// Allow empty calls to self.
			true,
			// These self calls are generally allowed.
			buildSupportedSignaturesCheck("invalid_self_call", [
				"function disableModule(address prevModule, address module)",
				"function addOwnerWithThreshold(address owner, uint256 threshold)",
				"function removeOwner(address prevOwner, address owner, uint256 threshold)",
				"function swapOwner(address prevOwner, address oldOwner, address newOwner)",
				"function changeThreshold(uint256 threshold)",
			]),
		),
	);
	// All base checks always have to pass
	const baseChecks = buildCombinedChecks([selfChecks, buildNoDelegateCallCheck()]);
	// Allowed delegate calls, otherwise fallback to base checks
	const allowedDelegateCalls = buildAddressSplitCheck(
		{
			...buildSingletonUpgradeChecks(),
			...buildSignMessageChecks(),
		},
		baseChecks,
	);
	// Add multisend checks, if not multisend, fallback to other allowed delegate calls
	const multiSendCheck150 = buildMultiSendCallOnlyCheck(allowedDelegateCalls, { toZeroIsSelf: true });
	const multiSendCheck = buildMultiSendCallOnlyCheck(allowedDelegateCalls);
	const supportedMultiSendChecks = buildAddressSplitCheck(
		{
			"0x218543288004CD07832472D464648173c77D7eB7": multiSendCheck150, // MultiSend - 1.5.0
			"0xA83c336B20401Af773B6219BA5027174338D1836": multiSendCheck150, // MultiSendCallOnly - 1.5.0
			"0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526": multiSendCheck, // MultiSend - 1.4.1
			"0x9641d764fc13c8B624c04430C7356C1C7C8102e2": multiSendCheck, // MultiSendCallOnly - 1.4.1
			"0xA238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761": multiSendCheck, // MultiSend - 1.3.0 - canonical
			"0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": multiSendCheck, // MultiSendCallOnly - 1.3.0 - canonical
			"0x998739BFdAAdde7C933B942a68053933098f9EDa": multiSendCheck, // MultiSend - 1.3.0 - eip155
			"0xA1dabEF33b3B82c7814B6D82A79e50F4AC44102B": multiSendCheck, // MultiSendCallOnly - 1.3.0 - eip155
		},
		allowedDelegateCalls,
	);
	return supportedMultiSendChecks;
};
