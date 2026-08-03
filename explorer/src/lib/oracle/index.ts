export * from "./abi";
export * from "./hashing";
export * from "./votes";
export * from "./votingStatus";

import { type Remote, wrap } from "comlink";
import type { OracleWorkerApi } from "./worker";

let instance: Remote<OracleWorkerApi> | undefined;

export function getOracleWorker(): Remote<OracleWorkerApi> {
	instance ??= wrap<OracleWorkerApi>(new Worker(new URL("./worker.ts", import.meta.url), { type: "module" }));
	return instance;
}
