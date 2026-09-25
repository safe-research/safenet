import { expose } from "comlink";
import type { Address, Hex } from "viem";
import { createRpcClient } from "@/lib/rpc";
import { loadArbitrator } from "./arbitrator";
import { loadSentinelVotes } from "./votes";
import { loadVotingStatus } from "./votingStatus";

type LoadVotesParams = {
	rpc: string;
	oracle: Address;
	consensus: Address;
	epoch: bigint;
	safeTxHash: Hex;
	oracleData: Hex;
	maxBlockRange: bigint;
};

const workerApi = {
	loadArbitrator: ({ rpc, oracle }: { rpc: string; oracle: Address }) =>
		loadArbitrator({ provider: createRpcClient(rpc), oracle }),
	loadVotingStatus: ({ rpc, ...params }: LoadVotesParams) =>
		loadVotingStatus({ ...params, provider: createRpcClient(rpc) }),
	loadSentinelVotes: ({ rpc, ...params }: LoadVotesParams) =>
		loadSentinelVotes({ ...params, provider: createRpcClient(rpc) }),
};

export type OracleWorkerApi = typeof workerApi;

expose(workerApi);
