import { type Address, type Hex, isAddressEqual } from "viem";

export type TransactionPayload = { to: Address; value: bigint; data: Hex };
export type Detector = (payload: TransactionPayload) => boolean;

export function createDetector(blocklist: readonly Address[] = []): Detector {
	return (payload) => !blocklist.some((blocked) => isAddressEqual(blocked, payload.to));
}
