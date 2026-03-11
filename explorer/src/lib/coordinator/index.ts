export * from "./keygen";
export * from "./signing";

import { type Remote, wrap } from "comlink";
import type { CoordinatorWorkerApi } from "./worker";

let instance: Remote<CoordinatorWorkerApi> | undefined;

export function getCoordinatorWorker(): Remote<CoordinatorWorkerApi> {
	instance ??= wrap<CoordinatorWorkerApi>(new Worker(new URL("./worker.ts", import.meta.url), { type: "module" }));
	return instance;
}
