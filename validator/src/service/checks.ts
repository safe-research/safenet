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
	// Following methods do not require additional parameter checks
	const unboundedSelfCallChecks = buildSupportedSignaturesCheck([
		"function disableModule(address prevModule, address module)",
		"function addOwnerWithThreshold(address owner, uint256 threshold)",
		"function removeOwner(address prevOwner, address owner, uint256 threshold)",
		"function swapOwner(address prevOwner, address oldOwner, address newOwner)",
		"function changeThreshold(uint256 threshold)",
	]);
	// Apply parameter checks for critical methods
	const selfChecks = buildSelfCheck(
		buildSelectorChecks(
			{
				...buildSetFallbackHandlerCheck(),
				...buildSetGuardCheck(),
				...buildSetModuleGuardCheck(),
				...buildEnableModuleCheck(),
			},
			true,
			unboundedSelfCallChecks,
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
	const multiSendCheck = buildMultiSendCallOnlyCheck(baseChecks);
	const supportedMultiSendChecks = buildAddressSplitCheck(
		{
			"0xA83c336B20401Af773B6219BA5027174338D1836": multiSendCheck,
			"0x9641d764fc13c8B624c04430C7356C1C7C8102e2": multiSendCheck,
			"0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": multiSendCheck,
		},
		allowedDelegateCalls,
	);
	return supportedMultiSendChecks;
};
