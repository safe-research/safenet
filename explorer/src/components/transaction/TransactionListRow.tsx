import { Link } from "@tanstack/react-router";
import { Badge } from "@/components/common/Badge";
import { NetworkBadge } from "@/components/common/NetworkBadge";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useConsensusState } from "@/hooks/useConsensusState";
import { shortAddress } from "@/lib/address";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { TransactionProposal } from "@/lib/consensus";
import { dataString, formatBlockAge, formatHashShort, opString, valueString } from "@/lib/safe/formatting";

/** Tailwind grid-cols definition shared with the header row in TransactionProposalsList. */
export const TRANSACTION_ROW_GRID_COLS = "grid grid-cols-[5rem_7.5rem_1fr_2fr_6rem] gap-x-2";

export function TransactionListRow({ proposal }: { proposal: TransactionProposal }) {
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
			<div
				className={`${TRANSACTION_ROW_GRID_COLS} items-start bg-surface-1 border border-surface-outline rounded-lg px-3 py-2.5 hover:bg-surface-hover cursor-pointer`}
			>
				{/* Column 1: Network + Status badges */}
				<div className="flex flex-col gap-1">
					<NetworkBadge chainId={proposal.chainId} />
					<StatusBadge attested={proposal.attestedAt !== null} />
				</div>

				{/* Column 2: Safe address */}
				<div className="text-xs font-mono text-sub-title truncate self-center">{shortAddress(tx.safe)}</div>

				{/* Column 3: SafeTxHash */}
				<div className="text-xs font-mono truncate self-center">{formatHashShort(proposal.safeTxHash)}</div>

				{/* Column 4: Operation badge + To / Summary */}
				<div className="min-w-0 self-center">
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
				<div className="text-xs text-right text-sub-title">
					{when && <div className="whitespace-nowrap">{when}</div>}
					<div className="font-mono whitespace-nowrap">#{proposal.proposedAt.block.toString()}</div>
				</div>
			</div>
		</Link>
	);
}
