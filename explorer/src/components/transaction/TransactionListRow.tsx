import { Link } from "@tanstack/react-router";
import { NetworkBadge } from "@/components/common/NetworkBadge";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useConsensusState } from "@/hooks/useConsensusState";
import { shortAddress } from "@/lib/address";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { TransactionProposal } from "@/lib/consensus";
import { dataString, formatBlockAge, formatHashShort, valueString } from "@/lib/safe/formatting";

export function TransactionListRow({ proposal }: { proposal: TransactionProposal }) {
	const { data: consensusState } = useConsensusState();
	const currentBlock = consensusState?.currentBlock ?? 0n;
	const chain = SAFE_SERVICE_CHAINS[proposal.chainId.toString()];

	const blockDiff = currentBlock > proposal.proposedAt.block ? currentBlock - proposal.proposedAt.block : 0n;
	const when = chain && blockDiff > 0n ? formatBlockAge(blockDiff, chain) : "";

	const { transaction: tx } = proposal;
	const summary = tx.value > 0n ? valueString(tx.value) : tx.data !== "0x" ? dataString(tx.data) : "(no data)";

	return (
		<Link to="/safeTx" search={{ chainId: `${proposal.chainId}`, safeTxHash: proposal.safeTxHash }}>
			<div className="flex items-start gap-3 bg-surface-1 border border-surface-outline rounded-lg p-3 hover:bg-surface-hover cursor-pointer">
				{/* Column 1: Network + Status badges */}
				<div className="flex flex-col gap-1 w-16 shrink-0">
					<NetworkBadge chainId={proposal.chainId} />
					<StatusBadge attested={proposal.attestedAt !== null} />
				</div>

				{/* Column 2: Safe address */}
				<div className="flex-1 min-w-0">
					<div className="text-xs font-mono text-sub-title truncate">{shortAddress(tx.safe)}</div>
				</div>

				{/* Column 3: SafeTxHash */}
				<div className="flex-1 min-w-0">
					<div className="text-xs font-mono truncate">{formatHashShort(proposal.safeTxHash)}</div>
				</div>

				{/* Column 4: To / Summary */}
				<div className="flex-1 min-w-0">
					<div className="text-xs truncate">to {shortAddress(tx.to)}</div>
					<div className="text-xs text-sub-title truncate">{summary}</div>
				</div>

				{/* Column 5: When */}
				<div className="text-xs text-right text-sub-title whitespace-nowrap w-16 shrink-0">{when}</div>
			</div>
		</Link>
	);
}
