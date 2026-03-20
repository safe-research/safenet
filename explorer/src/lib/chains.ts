import { arbitrum, base, type Chain, gnosis, gnosisChiado, mainnet, optimism, sepolia } from "viem/chains";

export type ChainInfo = Chain & {
	shortName: string;
	/** Brand background colour for network badges (hex, e.g. `"#0052FF"`). */
	color: string;
};

export const SAFE_SERVICE_CHAINS: Record<string, ChainInfo> = {
	"1": {
		...mainnet,
		shortName: "eth",
		color: "#3B5CE6",
	},
	"10": {
		...optimism,
		shortName: "oeth",
		color: "#FF0420",
	},
	"100": {
		...gnosis,
		shortName: "gno",
		color: "#04795B",
	},
	"8453": {
		...base,
		shortName: "base",
		color: "#0052FF",
		blockTime: 2_000,
	},
	"10200": {
		...gnosisChiado,
		shortName: "chi",
		color: "#2E6B52",
	},
	"42161": {
		...arbitrum,
		shortName: "arb1",
		color: "#28A0F0",
	},
	"11155111": {
		...sepolia,
		shortName: "sep",
		color: "#666666",
		blockTime: 12_000,
	},
};
