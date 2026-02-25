import { type Address, zeroAddress } from "viem";
import { buildSelectorCheck } from "../basic.js";
import { classifyTxCheck } from "../errors.js";

const ALLOWED_GUARDS: Address[] = [
	// No guards allowed right now!
];

export const buildSetGuardCheck = () =>
	classifyTxCheck(
		"unknown_guard",
		buildSelectorCheck("function setGuard(address)", ([guard]) => {
			if (guard !== zeroAddress && !ALLOWED_GUARDS.includes(guard)) {
				throw Error(`Cannot set unknown guard ${guard}`);
			}
		}),
	);
