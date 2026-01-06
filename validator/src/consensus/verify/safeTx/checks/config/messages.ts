import { toFunctionSelector } from "viem";
import type { TransactionCheck } from "../../handler.js";
import { buildFixedParamsCheck, buildSupportedSelectorCheck } from "../basic.js";
import { buildCombinedChecks } from "../combined.js";

const buildSignCheck = (): TransactionCheck =>
	buildCombinedChecks([
		buildFixedParamsCheck({ operation: 1 }),
		buildSupportedSelectorCheck([toFunctionSelector("function signMessage(bytes)")], false),
	]);

export const buildSignMessageChecks = (): Record<string, TransactionCheck> => {
	const signCheck = buildSignCheck();
	return {
		"0xA65387F16B013cf2Af4605Ad8aA5ec25a2cbA3a2": signCheck,
		"0x98FFBBF51bb33A056B08ddf711f289936AafF717": signCheck,
		"0xd53cd0aB83D845Ac265BE939c57F53AD838012c9": signCheck,
		"0x4FfeF8222648872B3dE295Ba1e49110E61f5b5aa": signCheck,
	};
};
