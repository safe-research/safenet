import { expose } from "comlink";
import type { Address, Hex } from "viem";
import { createRpcClient } from "@/lib/rpc";
import { loadKeyGenDetails } from "./keygen";
import { loadLatestAttestationStatus } from "./signing";

const workerApi = {
	loadLatestAttestationStatus: ({
		rpc,
		...params
	}: {
		rpc: string;
		consensus: Address;
		safeTxHash: Hex;
		epoch: bigint;
		proposedAt?: bigint;
		attestedAt?: bigint | null;
		maxBlockRange: bigint;
	}) => loadLatestAttestationStatus({ ...params, provider: createRpcClient(rpc) }),

	loadKeyGenDetails: ({
		rpc,
		...params
	}: {
		rpc: string;
		consensus: Address;
		gid: Hex;
		endBlock: bigint;
		blocksPerEpoch?: number;
		prevStagedAt?: bigint;
		maxBlockRange: bigint;
	}) => loadKeyGenDetails({ ...params, provider: createRpcClient(rpc) }),
};

export type CoordinatorWorkerApi = typeof workerApi;

expose(workerApi);
