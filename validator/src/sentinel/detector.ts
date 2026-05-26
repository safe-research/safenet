import { type Address, isAddressEqual } from "viem";
import type { TransactionPayload } from "./types.js";

export type Detector = (payload: TransactionPayload) => boolean;

export function createDetector(blocklist: readonly Address[] = []): Detector {
	return (payload) => !blocklist.some((blocked) => isAddressEqual(blocked, payload.to));
}
