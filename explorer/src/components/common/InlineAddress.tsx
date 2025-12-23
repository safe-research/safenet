import type { Address } from "viem";
import { shortAddress } from "@/lib/address";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function InlineAddress({
	chainId,
	address,
	disableLinks,
}: {
	chainId: bigint;
	address: Address;
	disableLinks?: boolean;
}) {
	const chainInfo = SAFE_SERVICE_CHAINS[chainId.toString()];
	const formattedAddress = shortAddress(address);
	if (disableLinks === true || chainInfo?.blockExplorers === undefined) {
		return <span className="font-mono">{formattedAddress}</span>;
	}
	const explorerLink = `${chainInfo.blockExplorers.default.url}/address/${address}`;
	return (
		<a href={explorerLink} target="_blank" rel="noopener noreferrer" className="font-mono">
			{formattedAddress}
		</a>
	);
}
