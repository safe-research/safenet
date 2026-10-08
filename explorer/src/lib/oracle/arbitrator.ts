import type { Address, PublicClient } from "viem";
import { sentinelOracleAbi } from "./abi";

// The account that rules on `SentinelOracle` disputes. It is immutable, so callers can cache it.
export const loadArbitrator = async ({ provider, oracle }: { provider: PublicClient; oracle: Address }) =>
	provider.readContract({
		address: oracle,
		abi: sentinelOracleAbi,
		functionName: "ARBITRATOR",
	});
