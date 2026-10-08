import type { Address, PublicClient } from "viem";
import { describe, expect, it, vi } from "vitest";
import { loadArbitrator } from "./arbitrator";

const ORACLE: Address = "0x1234567890123456789012345678901234567890";
const ARBITRATOR: Address = "0x9999999999999999999999999999999999999999";

describe("loadArbitrator", () => {
	it("reads ARBITRATOR from the oracle", async () => {
		const readContract = vi.fn().mockResolvedValue(ARBITRATOR);
		const provider = { readContract } as unknown as PublicClient;

		expect(await loadArbitrator({ provider, oracle: ORACLE })).toBe(ARBITRATOR);
		expect(readContract).toHaveBeenCalledWith(expect.objectContaining({ address: ORACLE, functionName: "ARBITRATOR" }));
	});
});
