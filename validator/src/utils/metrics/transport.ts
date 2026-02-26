import type { Transport } from "viem";
import type { Metrics } from "./index.js";

export const withMetrics = <T extends Transport>(transport: T, metrics: Metrics): T => {
	return ((options) => {
		const base = transport(options);
		return {
			...base,
			async request(args, options) {
				try {
					const response = await base.request(args, options);
					metrics.rpcRequests.labels({ method: args.method, result: "success" }).inc();
					return response;
				} catch (error) {
					metrics.rpcRequests.labels({ method: args.method, result: "failure" }).inc();
					throw error;
				}
			},
		};
	}) as T;
};
