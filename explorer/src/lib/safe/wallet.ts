import type { Address, Hex } from "viem";

export const safeWalletTxUrl = (shortName: string, safe: Address, safeTxHash: Hex): string =>
	`https://app.safe.global/transactions/tx?safe=${shortName}:${safe}&id=${safeTxHash}`;

export const safeWalletSafeUrl = (shortName: string, safe: Address): string =>
	`https://app.safe.global/balances?safe=${shortName}:${safe}`;
