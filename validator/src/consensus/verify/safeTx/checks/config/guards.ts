import { type Address, zeroAddress } from "viem";
import { buildSelectorCheck } from "../basic.js";
import { TransactionCheckError } from "../errors.js";

const ALLOWED_GUARDS: Address[] = [
	// No guards allowed right now!
];

export const buildSetGuardCheck = () =>
	buildSelectorCheck("function setGuard(address)", ([guard]) => {
		if (guard !== zeroAddress && !ALLOWED_GUARDS.includes(guard)) {
			throw new TransactionCheckError("unsupported_guard", `Cannot set unknown guard ${guard}`);
		}
	});
