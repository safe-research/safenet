import type { Address } from "viem";
import z from "zod";
import { checkedAddressSchema } from "@/lib/schemas";

export type SentinelInfo = {
	address: Address;
	label: string;
};

const sentinelInfoSchema = z.array(
	z.object({
		address: checkedAddressSchema,
		label: z.string(),
	}),
);

export const loadSentinelInfoMap = async (source: string): Promise<Map<Address, SentinelInfo>> => {
	return fetch(source).then(async (resp) => {
		if (!resp.ok) {
			throw new Error(`Failed to fetch sentinel info: ${resp.statusText}`);
		}
		return sentinelInfoSchema.parse(await resp.json()).reduce((map, info) => {
			map.set(info.address, info);
			return map;
		}, new Map<Address, SentinelInfo>());
	});
};
