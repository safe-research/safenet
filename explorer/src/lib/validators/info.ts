import type { Address } from "viem";
import z from "zod";
import { shortAddress } from "@/lib/address";
import { checkedAddressSchema } from "@/lib/schemas";

export type ValidatorInfo = {
	address: Address;
	label: string;
};

const validatorInfoSchema = z.array(
	z.object({
		address: checkedAddressSchema,
		label: z.string(),
	}),
);

export const loadValidatorInfoMap = async (source: string): Promise<Map<Address, ValidatorInfo>> => {
	return fetch(source).then(async (resp) => {
		if (!resp.ok) {
			throw new Error(`Failed to fetch validator info: ${resp.statusText}`);
		}
		return validatorInfoSchema.parse(await resp.json()).reduce((map, info) => {
			map.set(info.address, info);
			return map;
		}, new Map<Address, ValidatorInfo>());
	});
};

export const mapInfo = (
	validatorInfoMap: Map<Address, ValidatorInfo> | null | undefined,
	suffix: string,
	address: Address,
) => `${validatorInfoMap?.get(address)?.label ?? shortAddress(address)} ${suffix}`;

// Formats each address as active (✅) or, if inactive, as either still-pending (⏳)
// or missed (❌) depending on whether the roster has closed out.
export const createStatusMapInfo =
	(validatorInfoMap: Map<Address, ValidatorInfo> | null | undefined, completed: boolean) =>
	(address: Address, isActive: boolean) =>
		mapInfo(validatorInfoMap, isActive ? "✅" : completed ? "❌" : "⏳", address);
