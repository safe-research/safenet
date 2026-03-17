import { Link } from "@tanstack/react-router";
import { Badge } from "@/components/common/Badge";
import { NetworkBadge } from "@/components/common/NetworkBadge";
import { StatusBadge } from "@/components/common/StatusBadge";
import { Skeleton } from "@/components/Skeleton";
import { useConsensusState } from "@/hooks/useConsensusState";
import { shortAddress } from "@/lib/address";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { TransactionProposalWithStatus } from "@/lib/consensus";
import { dataString, formatBlockAge, formatHashShort, opString, valueString } from "@/lib/safe/formatting";

/** Grid wrapper shared by the header and data rows of the transaction list. */
export function TransactionRowGrid({ children, className }: { children: React.ReactNode; className?: string }) {
	return (
		<div
			className={`grid grid-cols-[1fr_auto] sm:grid-cols-[5rem_7.5rem_1fr_2fr_6rem] gap-x-2 gap-y-2 ${className ?? ""}`}
		>
			{children}
		</div>
	);
}

/** Placeholder row shown while the transaction list is loading. Matches the grid layout of TransactionListRow. */
export function TransactionListRowSkeleton() {
	return (
		<TransactionRowGrid className="items-start bg-surface-1 border border-surface-outline rounded-card px-3 py-2.5">
			<div className="flex flex-col gap-1">
				<Skeleton className="h-5 w-full" />
				<Skeleton className="h-5 w-full" />
			</div>
			<Skeleton className="col-span-2 sm:col-span-1 h-4 w-3/4" />
			<Skeleton className="hidden sm:block h-4 w-full" />
			<div className="col-span-2 sm:col-span-1 space-y-1">
				<Skeleton className="h-4 w-2/3" />
				<Skeleton className="h-4 w-full" />
			</div>
			<div className="col-start-2 row-start-1 sm:col-auto sm:row-auto flex flex-col gap-1 items-end">
				<Skeleton className="h-4 w-4/5" />
				<Skeleton className="h-4 w-full" />
			</div>
		</TransactionRowGrid>
	);
}

export function TransactionListRow({ proposal }: { proposal: TransactionProposalWithStatus }) {
	const { data: consensusState } = useConsensusState();
	const currentBlock = consensusState?.currentBlock ?? 0n;
	const chain = SAFE_SERVICE_CHAINS[proposal.chainId.toString()];

	const blockDiff = currentBlock > proposal.proposedAt.block ? currentBlock - proposal.proposedAt.block : 0n;
	const when = chain && blockDiff > 0n ? formatBlockAge(blockDiff, chain) : "";

	const { transaction: tx } = proposal;
	const summary = tx.value > 0n ? valueString(tx.value) : tx.data !== "0x" ? dataString(tx.data) : "(no data)";
	const isDelegateCall = tx.operation === 1;

	return (
		<Link to="/safeTx" search={{ chainId: `${proposal.chainId}`, safeTxHash: proposal.safeTxHash }}>
			<TransactionRowGrid className="items-start bg-surface-1 border border-surface-outline rounded-card px-3 py-2.5 hover:bg-secondary cursor-pointer">
				{/* Column 1: Network + Status badges */}
				<div className="flex flex-col gap-1">
					<NetworkBadge chainId={proposal.chainId} />
					<StatusBadge status={proposal.status} />
				</div>

				{/* Column 2: Safe address */}
				<div className="col-span-2 sm:col-span-1 text-xs font-mono text-sub-title truncate self-center">
					{shortAddress(tx.safe)}
				</div>

				{/* Column 3: SafeTxHash */}
				<div className="hidden sm:block text-xs font-mono truncate self-center">
					{formatHashShort(proposal.safeTxHash)}
				</div>

				{/* Column 4: Operation badge + To / Summary */}
				<div className="col-span-2 sm:col-span-1 min-w-0 self-center">
					<div className="flex items-center gap-1 min-w-0">
						<Badge
							className={`shrink-0 ${isDelegateCall ? "bg-warning-surface text-warning" : "bg-surface-outline text-title"}`}
						>
							{opString(tx.operation)}
						</Badge>
						<span className="text-xs truncate">to {shortAddress(tx.to)}</span>
					</div>
					<div className="text-xs text-sub-title truncate">{summary}</div>
				</div>

				{/* Column 5: When + Block number */}
				<div className="col-start-2 row-start-1 sm:col-auto sm:row-auto text-xs text-right text-sub-title">
					{when && <div className="whitespace-nowrap">{when}</div>}
					<div className="font-mono whitespace-nowrap">#{proposal.proposedAt.block.toString()}</div>
				</div>
			</TransactionRowGrid>
		</Link>
	);
}
