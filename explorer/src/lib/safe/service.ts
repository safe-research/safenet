import type { Hex } from "viem";
import z from "zod";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { SafeTransaction } from "@/lib/consensus";
import { calculateSafeTxHash } from "@/lib/safe/hashing";
import { bigIntSchema, checkedAddressSchema, hexDataSchema } from "@/lib/schemas";
import { loadSafeApiSettings } from "@/lib/settings";

const safeTransactionSchema = z.object({
	safeTxHash: hexDataSchema,
	safe: checkedAddressSchema,
	to: checkedAddressSchema,
	value: bigIntSchema,
	data: z.preprocess((v) => (typeof v !== "string" || v === "" ? "0x" : v), hexDataSchema),
	operation: z.union([z.literal(0), z.literal(1)]),
	safeTxGas: bigIntSchema,
	baseGas: bigIntSchema,
	gasPrice: bigIntSchema,
	gasToken: checkedAddressSchema,
	refundReceiver: checkedAddressSchema,
	nonce: bigIntSchema,
});

const buildSafeTxDetailsEndpoint = (base: string, shortName: string, safeTxHash: Hex) =>
	`${base}/tx-service/${shortName}/api/v2/multisig-transactions/${safeTxHash}/`;

export const loadSafeTransactionDetails = async (chainId: bigint, safeTxHash: Hex): Promise<SafeTransaction | null> => {
	const { url } = loadSafeApiSettings();
	const chainInfo = SAFE_SERVICE_CHAINS[chainId.toString()];
	if (chainInfo === undefined) return null;
	const response = await fetch(buildSafeTxDetailsEndpoint(url, chainInfo.shortName, safeTxHash));
	if (!response.ok) return null;
	const parsed = safeTransactionSchema.safeParse(await response.json());
	if (!parsed.success) return null;
	const transaction = {
		chainId,
		...parsed.data,
	};
	return calculateSafeTxHash(transaction) === safeTxHash ? transaction : null;
};
