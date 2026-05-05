import { InformationCircleIcon } from "@heroicons/react/24/outline";
import type { Hex } from "viem";
import { CopyButton } from "@/components/common/CopyButton";
import { InfoPopover } from "@/components/common/InfoPopover";
import { InlineHash } from "@/components/common/InlineHash";
import { StatusBadge } from "@/components/common/StatusBadge";
import { Box, BoxTitle } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { useProposalsForTransaction } from "@/hooks/useProposalsForTransaction";
import { useAttestationStatus } from "@/hooks/useSigningProgress";
import { useSubmitProposal } from "@/hooks/useSubmitProposal";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { SafeTransaction, TransactionProposal, TransactionProposalWithStatus } from "@/lib/consensus";
import { formatSignatureHex } from "@/lib/coordinator/signing";
import { InlineBlockInfo, InlineExplorerTxLink } from "../common/Info";
import { SafeTxAttestationStatus } from "./SafeTxAttestationStatus";

export function SafeTxProposals({ safeTxHash, transaction }: { safeTxHash: Hex; transaction: SafeTransaction }) {
	const proposals = useProposalsForTransaction(safeTxHash);

	return (
		<div className={"space-y-4"}>
			<BoxTitle>Transaction Proposals</BoxTitle>
			{proposals.isFetching && proposals.data.length === 0 && <Skeleton className="w-full h-25" />}
			{!proposals.isFetching && proposals.data.length === 0 && <NoSafeTxProposals transaction={transaction} />}
			{proposals.data.length !== 0 &&
				proposals.data.map((proposal, index) => (
					<div key={`${proposal.safeTxHash}:${proposal.epoch}`}>
						<SafeTxProposal proposal={proposal} number={index + 1} />
					</div>
				))}
		</div>
	);
}

function ProposalInfoButton({ proposal }: { proposal: TransactionProposal }) {
	const status = useAttestationStatus(
		proposal.safeTxHash,
		proposal.epoch,
		proposal.proposedAt.block,
		proposal.attestedAt?.block ?? null,
	);
	if (status.data === null) return null;

	if (!status.data.completed || !status.data.signature) {
		return (
			<InfoPopover trigger={<InformationCircleIcon className="h-4 w-4 text-muted" />}>
				<div className="grid grid-cols-[max-content_auto] items-center gap-x-2 gap-y-1">
					<span className="text-muted">Signature ID:</span>
					<div className="flex items-center gap-1 whitespace-nowrap">
						<InlineHash hash={status.data.sid} />
						<CopyButton value={status.data.sid} />
					</div>
					<span className="text-muted">Group ID:</span>
					<div className="flex items-center gap-1 whitespace-nowrap">
						<InlineHash hash={status.data.groupId} />
						<CopyButton value={status.data.groupId} />
					</div>
				</div>
			</InfoPopover>
		);
	}

	const signatureHex = formatSignatureHex(status.data.signature);

	return (
		<InfoPopover trigger={<InformationCircleIcon className="h-4 w-4 text-muted" />}>
			<div className="grid grid-cols-[max-content_auto] items-center gap-x-2 gap-y-1">
				<span className="text-muted">Signature ID:</span>
				<div className="flex items-center gap-1 whitespace-nowrap">
					<InlineHash hash={status.data.sid} />
					<CopyButton value={status.data.sid} />
				</div>
				<span className="text-muted">Group ID:</span>
				<div className="flex items-center gap-1 whitespace-nowrap">
					<InlineHash hash={status.data.groupId} />
					<CopyButton value={status.data.groupId} />
				</div>
				<span className="text-muted">Signature:</span>
				<div className="flex items-center gap-1 whitespace-nowrap">
					<InlineHash hash={signatureHex} />
					<CopyButton value={signatureHex} />
				</div>
			</div>
		</InfoPopover>
	);
}

function SafeTxProposal({ proposal, number }: { proposal: TransactionProposalWithStatus; number: number }) {
	return (
		<Box className="space-y-2">
			<div className="flex items-center gap-2">
				<p className="font-semibold">Proposal #{number}</p>
				<ProposalInfoButton proposal={proposal} />
			</div>
			<div className="md:flex md:justify-between">
				<p>Status:</p>
				<StatusBadge status={proposal.status} />
			</div>
			<div className="md:flex md:justify-between">
				<p className="mr-2">Proposed:</p>
				<p>
					<InlineBlockInfo block={proposal.proposedAt.block} />{" "}
					<InlineExplorerTxLink txHash={proposal.proposedAt.tx}>Explorer Tx</InlineExplorerTxLink>
				</p>
			</div>
			<div className="md:flex md:justify-between">
				<p className="mr-2">Attested:</p>
				<p>
					{proposal.attestedAt != null ? (
						<>
							<InlineBlockInfo block={proposal.attestedAt.block} />{" "}
							<InlineExplorerTxLink txHash={proposal.attestedAt.tx}>Explorer Tx</InlineExplorerTxLink>
						</>
					) : (
						"-"
					)}
				</p>
			</div>
			<SafeTxAttestationStatus proposal={proposal} />
		</Box>
	);
}

function NoSafeTxProposals({ transaction }: { transaction: SafeTransaction }) {
	const { enabled, mutation } = useSubmitProposal();
	const chain = SAFE_SERVICE_CHAINS[transaction.chainId.toString()];
	const chainName = chain?.name ?? `chain ${transaction.chainId}`;
	return (
		<Box className="flex w-full flex-col justify-center items-center space-y-4">
			<div>No proposals found for this SafeTxHash on {chainName}.</div>
			{enabled && !mutation.isSuccess && (
				<>
					<button
						type="button"
						className="px-4 py-2 border rounded-full bg-surface-1 hover:bg-secondary cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
						onClick={() => mutation.mutate(transaction)}
						disabled={mutation.isPending}
					>
						{mutation.isPending ? "Submitting" : "Submit Proposal"}
					</button>
					{mutation.error && <p className="text-error">{mutation.error.message}</p>}
				</>
			)}
		</Box>
	);
}
