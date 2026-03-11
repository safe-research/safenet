import { expose } from "comlink";
import { type Address, createPublicClient, type Hex, http } from "viem";
import {
	loadConsensusState,
	loadEpochRolloverHistory,
	loadEpochsState,
	loadProposedSafeTransaction,
	loadTransactionProposals,
} from "./consensus";

const createClient = (rpc: string) => createPublicClient({ transport: http(rpc) });

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
	}) => loadTransactionProposals({ ...params, provider: createClient(rpc) }),

	loadConsensusState: ({ rpc, consensus }: { rpc: string; consensus: Address }) =>
		loadConsensusState(createClient(rpc), consensus),

	loadProposedSafeTransaction: ({
		rpc,
		...params
	}: {
		rpc: string;
		consensus: Address;
		safeTxHash: Hex;
		maxBlockRange: bigint;
	}) => loadProposedSafeTransaction({ ...params, provider: createClient(rpc) }),

	loadEpochsState: ({ rpc, consensus }: { rpc: string; consensus: Address }) =>
		loadEpochsState(createClient(rpc), consensus),

	loadEpochRolloverHistory: ({
		rpc,
		...params
	}: {
		rpc: string;
		consensus: Address;
		maxBlockRange: bigint;
		cursor?: bigint;
	}) => loadEpochRolloverHistory({ ...params, provider: createClient(rpc) }),
};

export type ConsensusWorkerApi = typeof workerApi;

expose(workerApi);
