import { type Address, zeroAddress } from "viem";
import { buildSelectorCheck } from "../basic.js";

const ALLOWED_FALLBACK_HANDLERS: Address[] = [
	"0x85a8ca358D388530ad0fB95D0cb89Dd44Fc242c3", // ExtensibleFallbackHandler - 1.5.0
	"0x3EfCBb83A4A7AfcB4F68D501E2c2203a38be77f4", // CompatibilityFallbackHandler - 1.5.0
	"0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99", // CompatibilityFallbackHandler - 1.4.1
	"0xf48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4", // CompatibilityFallbackHandler - 1.3.0 - canonical
	"0x017062a1dE2FE6b99BE3d9d37841FeD19F573804", // CompatibilityFallbackHandler - 1.4.1 - eip155
];

export const buildSetFallbackHandlerCheck = () =>
	buildSelectorCheck("function setFallbackHandler(address)", ([handler]) => {
		if (handler !== zeroAddress && !ALLOWED_FALLBACK_HANDLERS.includes(handler)) {
			throw Error(`Cannot set unknown fallback handler ${handler}`);
		}
	});
