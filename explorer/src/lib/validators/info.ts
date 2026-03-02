import type { Address } from "viem";
import z from "zod";
import { checkedAddressSchema } from "@/lib/schemas";

export type ValidatorInfo = {
	identifier: bigint;
	address: Address;
	label: string;
};

const VALIDATOR_INFO_URL =
	"https://raw.githubusercontent.com/safe-fndn/safenet-validator-info/refs/heads/main/assets/safenet-validator-info.json";

const validatorInfoSchema = z.array(
	z.object({
		identifier: z.coerce.bigint(),
		address: checkedAddressSchema,
		label: z.string(),
	}),
);

export const loadValidatorInfoMap = async (): Promise<Map<bigint, ValidatorInfo>> => {
	return fetch(VALIDATOR_INFO_URL).then(async (resp) => {
		if (!resp.ok) {
			throw new Error(`Failed to fetch validator info: ${resp.statusText}`);
		}
		return validatorInfoSchema.parse(await resp.json()).reduce((map, info) => {
			map.set(info.identifier, info);
			return map;
		}, new Map<bigint, ValidatorInfo>());
	});
};
