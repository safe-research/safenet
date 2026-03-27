export type TransactionCheckErrorCode =
	| "no_delegatecall"
	| "unsupported_module"
	| "unsupported_module_guard"
	| "unsupported_guard"
	| "unsupported_fallback_handler"
	| "invalid_self_call"
	| "invalid_multisend"
	| "invalid_migration"
	| "invalid_sign_message"
	| "invalid_create_call"
	| "unknown";

export class TransactionCheckError extends Error {
	readonly code: TransactionCheckErrorCode;

	constructor(code: TransactionCheckErrorCode, message: string, options?: ErrorOptions) {
		super(message, options);
		this.name = "TransactionCheckError";
		this.code = code;
	}
}
