import { expose } from "comlink";
import type { Address, Hex } from "viem";
import { createRpcClient } from "@/lib/rpc";
import { loadSentinelVotes } from "./votes";
import { loadVotingStatus } from "./votingStatus";

type LoadVotesParams = {
	rpc: string;
	oracle: Address;
	consensus: Address;
	epoch: bigint;
	safeTxHash: Hex;
	maxBlockRange: bigint;
};

const workerApi = {
	loadVotingStatus: ({ rpc, ...params }: LoadVotesParams) =>
		loadVotingStatus({ ...params, provider: createRpcClient(rpc) }),
	loadSentinelVotes: ({ rpc, ...params }: LoadVotesParams) =>
		loadSentinelVotes({ ...params, provider: createRpcClient(rpc) }),
};

export type OracleWorkerApi = typeof workerApi;

expose(workerApi);
