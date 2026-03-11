import { ArrowTopRightOnSquareIcon } from "@heroicons/react/24/solid";
import type { Address, Hex } from "viem";
import { CopyButton } from "@/components/common/CopyButton";
import { BoxTitle } from "@/components/Groups";
import { useChainInfo } from "@/hooks/useChainInfo";
import { shortAddress } from "@/lib/address";
import type { ChainInfo } from "@/lib/chains";
import type { SafeTransaction } from "@/lib/consensus";
import { safeWalletSafeUrl, safeWalletTxUrl } from "@/lib/safe/wallet";

const shortHash = (hash: string): string => `${hash.slice(0, 6)}…${hash.slice(-4)}`;

function SafeWalletTxLink({ chainInfo, safe, safeTxHash }: { chainInfo: ChainInfo; safe: Address; safeTxHash: Hex }) {
	return (
		<a
			href={safeWalletTxUrl(chainInfo.shortName, safe, safeTxHash)}
			target="_blank"
			rel="noopener noreferrer"
			className="inline-flex items-center gap-1 text-sm hover:underline"
		>
			Open in Safe Wallet <ArrowTopRightOnSquareIcon className="inline-block h-4 w-4" />
		</a>
	);
}

function SafeWalletAccountLink({ chainInfo, safe }: { chainInfo: ChainInfo; safe: Address }) {
	return (
		<a
			href={safeWalletSafeUrl(chainInfo.shortName, safe)}
			target="_blank"
			rel="noopener noreferrer"
			className="inline-flex items-center gap-1 text-sm hover:underline"
		>
			Open in Safe Wallet <ArrowTopRightOnSquareIcon className="inline-block h-4 w-4" />
		</a>
	);
}

export const SafeTxHeader = ({
	safeTxHash,
	transaction,
	fromSafeApi,
}: {
	safeTxHash: Hex;
	transaction: SafeTransaction;
	fromSafeApi: boolean;
}) => {
	const chainInfo = useChainInfo(transaction.chainId);

	return (
		<div className="space-y-2">
			<BoxTitle>Safe Transaction</BoxTitle>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">SafeTxHash:</span>
				<span className="font-mono text-sm break-all">{shortHash(safeTxHash)}</span>
				<CopyButton value={safeTxHash} />
				{fromSafeApi && chainInfo !== undefined && (
					<SafeWalletTxLink chainInfo={chainInfo} safe={transaction.safe} safeTxHash={safeTxHash} />
				)}
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Network:</span>
				<span className="text-sm">
					{chainInfo !== undefined
						? `${chainInfo.name} (chainId ${transaction.chainId})`
						: `chainId ${transaction.chainId}`}
				</span>
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Safe:</span>
				<span className="font-mono text-sm">{shortAddress(transaction.safe)}</span>
				<CopyButton value={transaction.safe} />
				{chainInfo !== undefined && <SafeWalletAccountLink chainInfo={chainInfo} safe={transaction.safe} />}
			</div>
		</div>
	);
};
