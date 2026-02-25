import type { Transport } from "viem";
import type { Metrics } from "./metrics.js";

export const wrapTransportWithRpcMetrics = (transport: Transport, metrics: Metrics): Transport => {
	return (options) => {
		const base = transport(options);
		return {
			...base,
			request: async (args, requestOptions) => {
				try {
					const response = await base.request(args, requestOptions);
					metrics.rpcRequests.labels({ method: args.method, result: "success" }).inc();
					return response;
				} catch (error) {
					metrics.rpcRequests.labels({ method: args.method, result: "failure" }).inc();
					throw error;
				}
			},
		};
	};
};
