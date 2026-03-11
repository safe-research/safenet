import { ArrowTopRightOnSquareIcon } from "@heroicons/react/24/solid";
import type { Hex } from "viem";
import { CopyButton } from "@/components/common/CopyButton";
import { BoxTitle } from "@/components/Groups";
import { shortAddress } from "@/lib/address";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { SafeTransaction } from "@/lib/consensus";
import { safeWalletSafeUrl, safeWalletTxUrl } from "@/lib/safe/wallet";

const shortHash = (hash: string): string => `${hash.slice(0, 6)}…${hash.slice(-4)}`;

export const SafeTxHeader = ({
	safeTxHash,
	transaction,
	fromSafeApi,
}: {
	safeTxHash: Hex;
	transaction: SafeTransaction;
	fromSafeApi: boolean;
}) => {
	const chainIdString = `${transaction.chainId}`;
	const chainInfo = SAFE_SERVICE_CHAINS[chainIdString];
	const chainName = chainInfo?.name ?? chainIdString;
	const showSafeWalletLinks = chainInfo !== undefined;

	return (
		<div className="space-y-2">
			<BoxTitle>Safe TX</BoxTitle>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">SafeTxHash:</span>
				<span className="font-mono text-sm break-all">{shortHash(safeTxHash)}</span>
				<CopyButton value={safeTxHash} />
				{fromSafeApi && showSafeWalletLinks && (
					<a
						href={safeWalletTxUrl(chainInfo.shortName, transaction.safe, safeTxHash)}
						target="_blank"
						rel="noopener noreferrer"
						className="inline-flex items-center gap-1 text-sm hover:underline"
					>
						Open in Safe Wallet <ArrowTopRightOnSquareIcon className="inline-block h-4 w-4" />
					</a>
				)}
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Network:</span>
				<span className="text-sm">
					{chainName} (chainId {chainIdString})
				</span>
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Safe:</span>
				<span className="font-mono text-sm">{shortAddress(transaction.safe)}</span>
				<CopyButton value={transaction.safe} />
				{showSafeWalletLinks && (
					<a
						href={safeWalletSafeUrl(chainInfo.shortName, transaction.safe)}
						target="_blank"
						rel="noopener noreferrer"
						className="inline-flex items-center gap-1 text-sm hover:underline"
					>
						Open in Safe Wallet <ArrowTopRightOnSquareIcon className="inline-block h-4 w-4" />
					</a>
				)}
			</div>
		</div>
	);
};
