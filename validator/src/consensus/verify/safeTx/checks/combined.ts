import type { TransactionCheck } from "../handler.js";
import type { MetaTransaction } from "../schemas.js";

export class CombinedChecks implements TransactionCheck {
	constructor(private checks: TransactionCheck[]) {}

	check(tx: MetaTransaction): void {
		for (const check of this.checks) {
			check.check(tx);
		}
	}
}

export class AddressSplitCheck implements TransactionCheck {
	constructor(private checks: Record<string, TransactionCheck>, private fallback?: TransactionCheck) {}

	check(tx: MetaTransaction): void {
		// First check for chain specific check and then fallback to chain independent checks
		const toWithPrefix = `eip155:${tx.chainId}:${tx.to}`;
		const check = this.checks[toWithPrefix] ?? this.checks[tx.to] ?? this.fallback;
		if (check === undefined) {
			throw new Error(`Delegatecalls for ${toWithPrefix} not allowed!`);
		}
		check.check(tx);
	}
}
