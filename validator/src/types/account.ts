import type { LocalAccount } from "viem/accounts";

/**
 * A validator EVM account.
 *
 * Currently, the validator account only needs a well-known address and to be
 * able to sign transactions.
 */
export type ValidatorAccount = Pick<LocalAccount, "address" | "signTransaction">;
