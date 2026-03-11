import { createPublicClient, http, type PublicClient } from "viem";

export const createRpcClient = (rpc: string): PublicClient => createPublicClient({ transport: http(rpc) });
