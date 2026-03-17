import { BaseError, RpcError } from "viem";

export const formatError = (err: unknown): unknown => {
	if (err instanceof BaseError) {
		// Use .walk() to find an error with a stack trace
		const ground0 = err.walk((err) => !!err && typeof err === "object" && "stack" in err && err.stack !== undefined);

		return {
			message: err.shortMessage || err.message,
			details: err.details || "No additional details",
			name: err.name,
			stack: ground0?.stack,
			// Exclude 'cause' to keep JSON logs single-line
		};
	}

	return err;
};

export const formatRpcError = (err: unknown): unknown => {
	if (err instanceof RpcError) {
		return { message: err.shortMessage, code: err.code };
	}
	if (err instanceof Error) {
		return { message: err.message };
	}
	return { message: "Unknown error" };
};
