import type { Address } from "viem";
import { shortAddress } from "@/lib/address";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function InlineAddress({ chainId, address }: { chainId: bigint; address: Address }) {
	const chainInfo = SAFE_SERVICE_CHAINS[chainId.toString()];
	const formattedAddress = shortAddress(address);
	if (chainInfo?.blockExplorers === undefined) {
		return <p className="font-mono">{formattedAddress}</p>;
	}
	const explorerLink = `${chainInfo.blockExplorers.default.url}/address/${address}`;
	return (
		<a href={explorerLink} target="_blank" className="font-mono">
			{formattedAddress}
		</a>
	);
}
