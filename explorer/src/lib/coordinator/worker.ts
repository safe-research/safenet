import { expose } from "comlink";
import { type Address, createPublicClient, type Hex, http } from "viem";
import { loadKeyGenDetails } from "./keygen";
import { loadLatestAttestationStatus } from "./signing";

const createClient = (rpc: string) => createPublicClient({ transport: http(rpc) });

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
	}) => loadLatestAttestationStatus({ ...params, provider: createClient(rpc) }),

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
	}) => loadKeyGenDetails({ ...params, provider: createClient(rpc) }),
};

export type CoordinatorWorkerApi = typeof workerApi;

expose(workerApi);
