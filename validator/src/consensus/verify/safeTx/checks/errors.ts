import type { TransactionCheck } from "../handler.js";
import type { SafeTransaction } from "../schemas.js";

export type TransactionCheckErrorCode =
	| "no_delegatecall"
	| "unknown_module"
	| "unknown_module_guard"
	| "unknown_guard"
	| "unknown_fallback_handler"
	| "invalid_self_call"
	| "invalid_multisend"
	| "invalid_migration"
	| "invalid_sign_message";

export class TransactionCheckError extends Error {
	readonly code: TransactionCheckErrorCode;

	constructor(code: TransactionCheckErrorCode, message: string, options?: ErrorOptions) {
		super(message, options);
		this.name = "TransactionCheckError";
		this.code = code;
	}
}

export function classifyTxCheck(code: TransactionCheckErrorCode, check: TransactionCheck): TransactionCheck;
export function classifyTxCheck(code: TransactionCheckErrorCode, check: TransactionCheck[]): TransactionCheck[];
export function classifyTxCheck(
	code: TransactionCheckErrorCode,
	check: Record<string, TransactionCheck>,
): Record<string, TransactionCheck>;
export function classifyTxCheck(
	code: TransactionCheckErrorCode,
	check: TransactionCheck | TransactionCheck[] | Record<string, TransactionCheck>,
): TransactionCheck | TransactionCheck[] | Record<string, TransactionCheck> {
	if (Array.isArray(check)) {
		return check.map((c) => classifyTxCheck(code, c));
	}
	if (typeof check === "object") {
		return Object.fromEntries(Object.entries(check).map(([key, c]) => [key, classifyTxCheck(code, c)]));
	}
	return (tx: SafeTransaction) => {
		try {
			check(tx);
		} catch (err) {
			if (err instanceof TransactionCheckError) {
				throw err;
			}
			const message = err instanceof Error ? err.message : "Unknown transaction check error";
			throw new TransactionCheckError(code, message);
		}
	};
}
