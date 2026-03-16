import type { Address } from "viem";
import z from "zod";
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
