import { useMemo } from "react";
import { Box, BoxTitle } from "@/components/Groups";
import { useProposalsForTransaction } from "@/hooks/useProposalsForTransaction";
import { useSubmitProposal } from "@/hooks/useSubmitProposal";
import type { SafeTransaction, TransactionProposal } from "@/lib/consensus";
import { calculateSafeTxHash } from "@/lib/safe/hashing";
import { Skeleton } from "../Skeleton";

export function SafeTxProposals({ transaction }: { transaction: SafeTransaction }) {
	const safeTxHash = useMemo(() => {
		return calculateSafeTxHash(transaction);
	}, [transaction]);
	const proposals = useProposalsForTransaction(safeTxHash);

	return (
		<div className={"space-y-4"}>
			<BoxTitle>Transaction Proposals</BoxTitle>
			{proposals.isLoading && <Skeleton className="w-full h-25" />}
			{!proposals.isLoading && proposals.data.length === 0 && <NoSafeTxProposals transaction={transaction} />}
			{!proposals.isLoading &&
				proposals.data.length !== 0 &&
				proposals.data.map((proposal) => (
					<div key={`${proposal.safeTxHash}:${proposal.epoch}`}>
						<SafeTxProposal proposal={proposal} />
					</div>
				))}
		</div>
	);
}

function SafeTxProposal({ proposal }: { proposal: TransactionProposal }) {
	const isAttested = proposal.attestedAt !== null;
	return (
		<Box className={`${isAttested ? "border-positive" : "border-pending"}`}>
			<p>Proposed at block {proposal.proposedAt}</p>
			{isAttested && <p>Attested at block {proposal.attestedAt}</p>}
			{!isAttested && <p>Attestation pending</p>}
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
