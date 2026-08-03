import { expose } from "comlink";
import type { Address, Hex } from "viem";
import { createRpcClient } from "@/lib/rpc";
import { loadVotingStatus } from "./votingStatus";

const workerApi = {
	loadVotingStatus: ({ rpc, ...params }: { rpc: string; oracle: Address; requestId: Hex; maxBlockRange: bigint }) =>
		loadVotingStatus({ ...params, provider: createRpcClient(rpc) }),
};

export type OracleWorkerApi = typeof workerApi;

expose(workerApi);
