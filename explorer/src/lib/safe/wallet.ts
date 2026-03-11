import type { Address, Hex } from "viem";

const SAFE_WALLET_BASE_URL = "https://app.safe.global";

export const safeWalletTxUrl = (shortName: string, safe: Address, safeTxHash: Hex): string =>
	`${SAFE_WALLET_BASE_URL}/transactions/tx?safe=${shortName}:${safe}&id=${safeTxHash}`;

export const safeWalletSafeUrl = (shortName: string, safe: Address): string =>
	`${SAFE_WALLET_BASE_URL}/balances?safe=${shortName}:${safe}`;
