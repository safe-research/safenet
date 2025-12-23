import { useMemo } from "react";
import { createPublicClient, http, type PublicClient } from "viem";
import { useSettings } from "./useSettings";

export function useProvider(): PublicClient {
	const [settings] = useSettings();
	const provider = useMemo(() => {
		return createPublicClient({
			transport: http(settings.rpc),
		});
	}, [settings.rpc]);
	return provider;
}
