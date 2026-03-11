import { expose } from "comlink";
import type { Address, Hex } from "viem";
import { createRpcClient } from "@/lib/rpc";
import {
	loadConsensusState,
	loadEpochRolloverHistory,
	loadEpochsState,
	loadProposedSafeTransaction,
	loadTransactionProposals,
} from "./consensus";

const workerApi = {
	loadTransactionProposals: ({
		rpc,
		...params
	}: {
		rpc: string;
		consensus: Address;
		safeTxHash?: Hex;
		safe?: Address;
		toBlock?: bigint;
		maxBlockRange: bigint;
	}) => loadTransactionProposals({ ...params, provider: createRpcClient(rpc) }),

	loadConsensusState: ({ rpc, consensus }: { rpc: string; consensus: Address }) =>
		loadConsensusState(createRpcClient(rpc), consensus),

	loadProposedSafeTransaction: ({
		rpc,
		...params
	}: {
		rpc: string;
		consensus: Address;
		safeTxHash: Hex;
		maxBlockRange: bigint;
	}) => loadProposedSafeTransaction({ ...params, provider: createRpcClient(rpc) }),

	loadEpochsState: ({ rpc, consensus }: { rpc: string; consensus: Address }) =>
		loadEpochsState(createRpcClient(rpc), consensus),

	loadEpochRolloverHistory: ({
		rpc,
		...params
	}: {
		rpc: string;
		consensus: Address;
		maxBlockRange: bigint;
		cursor?: bigint;
	}) => loadEpochRolloverHistory({ ...params, provider: createRpcClient(rpc) }),
};

export type ConsensusWorkerApi = typeof workerApi;

expose(workerApi);
