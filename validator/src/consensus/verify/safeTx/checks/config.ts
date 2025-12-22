import type { TransactionCheck } from "../handler.js";
import type { MetaTransaction } from "../schemas.js";

export class SafeConfigCheck implements TransactionCheck {
	check(tx: MetaTransaction): void {
		// Config changes are transactions to the account itself
		if (tx.account !== tx.to) return;
		if (tx.operation !== 0) throw new Error("Delegatecall not allowed");
		throw new Error("Method not implemented.");
	}
}
