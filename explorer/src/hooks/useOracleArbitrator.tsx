import { useQuery } from "@tanstack/react-query";
import type { Address } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getOracleWorker } from "@/lib/oracle";

// `ARBITRATOR` is immutable, so it is read once and never refetched. There is no `initialData`,
// because react-query treats it as fresh and would then never fetch: `data` is `undefined`
// until the read completes. `enabled` should be false unless the proposal has an arbitration,
// since only a `SentinelOracle` has an arbitrator to read.
export function useOracleArbitrator(oracle: Address, enabled = false) {
	const [settings] = useSettings();
	return useQuery<Address, Error>({
		queryKey: ["oracleArbitrator", settings.rpc, oracle],
		queryFn: () => getOracleWorker().loadArbitrator({ rpc: settings.rpc, oracle }),
		enabled,
		staleTime: Number.POSITIVE_INFINITY,
	});
}
