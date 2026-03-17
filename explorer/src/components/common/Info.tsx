import { ArrowTopRightOnSquareIcon } from "@heroicons/react/24/solid";
import type { Hex } from "viem";
import { useBlockInfo, useChainId } from "@/hooks/useProvider";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function InlineExplorerTxLink({
	children,
	txHash,
	hideWithoutLink = false,
}: {
	children: React.ReactNode;
	txHash: Hex;
	hideWithoutLink?: boolean;
}) {
	const chainId = useChainId();
	const chainInfo = chainId.data === null ? null : SAFE_SERVICE_CHAINS[chainId.data.toString()];
	if (chainInfo?.blockExplorers === undefined && hideWithoutLink) return;
	if (chainInfo?.blockExplorers === undefined) return children;
	const explorerLink = `${chainInfo.blockExplorers.default.url}/tx/${txHash}`;
	return (
		<>
			[
			<a href={explorerLink} target="_blank" rel="noopener noreferrer">
				{children} <ArrowTopRightOnSquareIcon className="inline-block h-4 w-4 mb-1" />
			</a>
			]
		</>
	);
}

export function InlineExplorerBlockLink({
	children,
	blockNumber,
	hideWithoutLink = false,
}: {
	children?: React.ReactNode;
	blockNumber: bigint;
	hideWithoutLink?: boolean;
}) {
	const chainId = useChainId();
	const chainInfo = chainId.data === null ? null : SAFE_SERVICE_CHAINS[chainId.data.toString()];
	if (chainInfo?.blockExplorers === undefined && hideWithoutLink) return;
	if (chainInfo?.blockExplorers === undefined) return children;
	const explorerLink = `${chainInfo.blockExplorers.default.url}/block/${blockNumber}`;
	return (
		<>
			[
			<a href={explorerLink} target="_blank" rel="noopener noreferrer">
				{children} <ArrowTopRightOnSquareIcon className="inline-block h-4 w-4 mb-1" />
			</a>
			]
		</>
	);
}

export function InlineBlockInfo({ block }: { block: bigint }) {
	const blockInfo = useBlockInfo(block);
	if (blockInfo.data === null) {
		return <span className="font-mono">Block {block}</span>;
	}
	const date = new Date(Number(blockInfo.data.timestamp * 1000n));
	return (
		<span className="font-mono">
			Block {block} at {date.toLocaleString()}
		</span>
	);
}
