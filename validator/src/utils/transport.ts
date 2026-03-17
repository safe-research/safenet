import type { Transport } from "viem";
import { formatRpcError } from "./errors.js";
import type { Logger } from "./logging.js";
import type { Metrics } from "./metrics.js";

export type TransportTracingParameters = {
	logger: Logger;
	metrics: Metrics;
};

/**
 * Wraps a Viem transport with additional tracing.
 *
 * This allows us to track additional data on how an RPC provider is responding
 * to requests and diagnose RPC issues.
 */
export const withTracing = <T extends Transport>(transport: T, { logger, metrics }: TransportTracingParameters): T => {
	return ((options) => {
		const base = transport(options);
		return {
			...base,
			async request(args, options) {
				try {
					const response = await base.request(args, options);

					logger.silly("JSON RPC request", { request: args, response });
					metrics.rpcRequests.labels({ method: args.method, result: "success" }).inc();
					return response;
				} catch (error) {
					logger.silly("JSON RPC request", { request: args, error: formatRpcError(error) });
					metrics.rpcRequests.labels({ method: args.method, result: "failure" }).inc();
					throw error;
				}
			},
		};
	}) as T;
};
