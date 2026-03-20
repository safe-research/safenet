import { createPublicClient, http, type PublicClient } from "viem";

let currentProvider:
	| {
			rpc: string;
			client: PublicClient;
	  }
	| undefined;

export const createRpcClient = (rpc: string): PublicClient => {
	if (currentProvider?.rpc !== rpc) {
		currentProvider = {
			rpc,
			client: createPublicClient({ transport: http(rpc) }),
		};
	}
	return currentProvider.client;
};
