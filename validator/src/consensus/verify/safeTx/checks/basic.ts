import { TransactionCheck } from "../handler.js";
import { MetaTransaction } from "../schemas.js";

export class NoDelegateCallCheck implements TransactionCheck {
    check(tx: MetaTransaction): void {
        if (tx.operation !== 0) throw new Error("Delegatecall not allowed");
    }
}

export class FixedParamsCheck implements TransactionCheck {

    constructor(private params: Partial<MetaTransaction>) {}

    check(tx: MetaTransaction): void {
        if (this.params.operation != undefined && tx.operation !== this.params.operation) {
            throw new Error(`Expected operation ${this.params.operation} got ${tx.operation}`);
        }
        if (this.params.to != undefined && tx.to !== this.params.to) {
            throw new Error(`Expected to ${this.params.to} got ${tx.to}`);
        }
        if (this.params.data != undefined && tx.data !== this.params.data) {
            throw new Error(`Expected data ${this.params.data} got ${tx.data}`);
        }
        if (this.params.value != undefined && tx.value !== this.params.value) {
            throw new Error(`Expected value ${this.params.value} got ${tx.value}`);
        }
    }
}
