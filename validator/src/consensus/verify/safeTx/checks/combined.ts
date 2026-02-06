import type { TransactionCheck } from "../handler.js";
import type { SafeTransaction } from "../schemas.js";

export const buildCombinedChecks = (checks: readonly TransactionCheck[]) => (tx: SafeTransaction) => {
	for (const check of checks) {
		check(tx);
	}
};

export const buildAddressSplitCheck =
	(checks: Readonly<Record<string, TransactionCheck>>, fallback?: TransactionCheck) => (tx: SafeTransaction) => {
		// First check for chain specific check and then fallback to chain independent checks
		const toWithPrefix = `eip155:${tx.chainId}:${tx.to}`;
		const check = checks[toWithPrefix] ?? checks[tx.to] ?? fallback;
		check?.(tx);
	};
