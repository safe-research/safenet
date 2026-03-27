import { toFunctionSelector } from "viem";
import type { TransactionCheck } from "../../handler.js";
import { buildFixedParamsCheck, buildSupportedSelectorCheck } from "../basic.js";
import { buildCombinedChecks } from "../combined.js";

const buildCreateCallCheck = (): TransactionCheck =>
	buildCombinedChecks([
		buildFixedParamsCheck("invalid_create_call", { operation: 1 }),
		buildSupportedSelectorCheck(
			"invalid_create_call",
			[
				toFunctionSelector("function performCreate(uint256,bytes)"),
				toFunctionSelector("function performCreate2(uint256,bytes,bytes32)"),
			],
			false,
		),
	]);

export const buildCreateCallChecks = (): Record<string, TransactionCheck> => {
	const createCallCheck = buildCreateCallCheck();
	return {
		"0x7cbB62EaA69F79e6873cD1ecB2392971036cFAa4": createCallCheck, // 1.3.0 - canonical
		"0xB19D6FFc2182150F8Eb585b79D4ABcd7C5640A9d": createCallCheck, // 1.3.0 - eip155
		"0x9b35Af71d77eaf8d7e40252370304687390A1A52": createCallCheck, // 1.4.1
		"0x2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4": createCallCheck, // 1.5.0
	};
};
