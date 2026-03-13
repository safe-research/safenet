import { ArrowTopRightOnSquareIcon } from "@heroicons/react/24/solid";
import type { Address, Hex } from "viem";
import { CopyButton } from "@/components/common/CopyButton";
import { NetworkBadge } from "@/components/common/NetworkBadge";
import { BoxTitle } from "@/components/Groups";
import { useChainInfo } from "@/hooks/useChainInfo";
import { shortAddress } from "@/lib/address";
import type { ChainInfo } from "@/lib/chains";
import type { SafeTransaction } from "@/lib/consensus";
import { safeWalletSafeUrl, safeWalletTxUrl } from "@/lib/safe/wallet";

const shortHash = (hash: string): string => `${hash.slice(0, 6)}…${hash.slice(-4)}`;

function SafeWalletLink({ href }: { href: string }) {
	return (
		<a
			href={href}
			target="_blank"
			rel="noopener noreferrer"
			className="inline-flex items-center gap-1 text-sm hover:underline"
		>
			Open in Safe Wallet <ArrowTopRightOnSquareIcon className="inline-block h-4 w-4" />
		</a>
	);
}

function SafeWalletTxLink({ chainInfo, safe, safeTxHash }: { chainInfo: ChainInfo; safe: Address; safeTxHash: Hex }) {
	return <SafeWalletLink href={safeWalletTxUrl(chainInfo.shortName, safe, safeTxHash)} />;
}

function SafeWalletAccountLink({ chainInfo, safe }: { chainInfo: ChainInfo; safe: Address }) {
	return <SafeWalletLink href={safeWalletSafeUrl(chainInfo.shortName, safe)} />;
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
	const networkTooltip =
		chainInfo !== undefined ? `${chainInfo.name} (chain id ${transaction.chainId})` : `chain id ${transaction.chainId}`;

	return (
		<div className="space-y-2">
			<BoxTitle>Safe Transaction</BoxTitle>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">SafeTxHash:</span>
				<span className="font-mono text-sm leading-none mt-1">{shortHash(safeTxHash)}</span>
				<CopyButton value={safeTxHash} />
				{fromSafeApi && chainInfo !== undefined && (
					<SafeWalletTxLink chainInfo={chainInfo} safe={transaction.safe} safeTxHash={safeTxHash} />
				)}
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Network:</span>
				<NetworkBadge chainId={transaction.chainId} title={networkTooltip} />
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Safe:</span>
				<span className="font-mono text-sm leading-none mt-1">{shortAddress(transaction.safe)}</span>
				<CopyButton value={transaction.safe} />
				{chainInfo !== undefined && <SafeWalletAccountLink chainInfo={chainInfo} safe={transaction.safe} />}
			</div>
		</div>
	);
};
