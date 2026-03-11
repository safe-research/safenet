export * from "./abi";
export * from "./epochs";
export * from "./transactions";

import { type Remote, wrap } from "comlink";
import type { ConsensusWorkerApi } from "./worker";

let instance: Remote<ConsensusWorkerApi> | undefined;

export function getConsensusWorker(): Remote<ConsensusWorkerApi> {
	instance ??= wrap<ConsensusWorkerApi>(new Worker(new URL("./worker.ts", import.meta.url), { type: "module" }));
	return instance;
}
