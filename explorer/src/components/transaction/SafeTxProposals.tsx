import type { Hex } from "viem";
import { Box, BoxTitle } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { useProposalsForTransaction } from "@/hooks/useProposalsForTransaction";
import { useSubmitProposal } from "@/hooks/useSubmitProposal";
import type { SafeTransaction, TransactionProposal } from "@/lib/consensus";
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
				proposals.data.map((proposal) => (
					<div key={`${proposal.safeTxHash}:${proposal.epoch}`}>
						<SafeTxProposal proposal={proposal} />
					</div>
				))}
		</div>
	);
}

function SafeTxProposal({ proposal }: { proposal: TransactionProposal }) {
	return (
		<Box className={`${proposal.attestedAt !== null ? "border-positive" : "border-pending"} space-y-2`}>
			<div className="md:flex md:justify-between">
				<p>Status:</p>
				<p>{proposal.attestedAt !== null ? "ATTESTED" : "PROPOSED"}</p>
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
	return (
		<Box className="flex w-full flex-col justify-center items-center space-y-4">
			<div>No proposals for this transaction!</div>
			{enabled && !mutation.isSuccess && (
				<>
					<button
						type="button"
						className="px-4 py-2 border rounded-full bg-surface-1 hover:bg-surface-hover cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed "
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
