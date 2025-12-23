import { base, type Chain, gnosis, gnosisChiado, mainnet, sepolia } from "viem/chains";

export type ChainInfo = Chain & {
	shortName: string;
};

export const SAFE_SERVICE_CHAINS: Record<string, ChainInfo> = {
	"1": {
		...mainnet,
		shortName: "eth",
	},
	"100": {
		...gnosis,
		shortName: "gno",
	},
	"8453": {
		...base,
		shortName: "base",
	},
	"11155111": {
		...sepolia,
		shortName: "sep",
	},
	"10200": {
		...gnosisChiado,
		shortName: "chi",
	},
};
