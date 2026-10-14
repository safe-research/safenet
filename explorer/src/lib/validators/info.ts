import type { Address } from "viem";
import z from "zod";
import { checkedAddressSchema } from "@/lib/schemas";
import { mapAddressLabel } from "@/lib/utils";

export type ValidatorInfo = {
	address: Address;
	label: string;
	isActive: boolean;
};

const validatorInfoSchema = z.array(
	z
		.object({
			address: checkedAddressSchema,
			label: z.string(),
			is_active: z.boolean().default(true),
		})
		.transform(({ is_active, ...info }) => ({ ...info, isActive: is_active })),
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

export const activeValidators = (validatorInfoMap: Map<Address, ValidatorInfo> | null | undefined): Address[] =>
	Array.from(validatorInfoMap?.values() ?? [])
		.filter((info) => info.isActive)
		.map((info) => info.address);

// Formats each address as active (✅) or, if inactive, as either still-pending (⏳)
// or missed (❌) depending on whether the roster has closed out.
export const createStatusMapInfo =
	(validatorInfoMap: Map<Address, ValidatorInfo> | null | undefined, completed: boolean) =>
	(address: Address, isActive: boolean) =>
		mapAddressLabel(validatorInfoMap, isActive ? "✅" : completed ? "❌" : "⏳", address);
