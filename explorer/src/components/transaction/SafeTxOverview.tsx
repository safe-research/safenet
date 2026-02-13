import { Link } from "@tanstack/react-router";
import { useMemo } from "react";
import type { Hex } from "viem";
import { useProposalsForTransaction } from "@/hooks/useProposalsForTransaction";
import { useSettings } from "@/hooks/useSettings";
import { useSubmitProposal } from "@/hooks/useSubmitProposal";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { SafeTransaction } from "@/lib/consensus";
import { dataString, opString, valueString } from "@/lib/safe/formatting";
import { calculateSafeTxHash } from "@/lib/safe/hashing";
import { InlineAddress } from "../common/InlineAddress";
import { Box, BoxTitle } from "../Groups";
import { TransactionProposalDetails } from "./Proposals";

export const SafeTxOverview = ({
	title,
	transaction,
	timestamp,
	disableLinks,
}: {
	title: string;
	transaction: SafeTransaction;
	timestamp?: string;
	disableLinks?: boolean;
}) => {
	const chainIdString = `${transaction.chainId}`;
	const chainInfo = SAFE_SERVICE_CHAINS[chainIdString];
	const chainName = chainInfo?.name ?? chainIdString;
	return (
		<>
			<div className={"flex justify-between"}>
				<p className={"text-xs"}>{title}</p>
				{timestamp !== undefined && <p className={"text-xs"}>{timestamp}</p>}
			</div>
			<div>
				<InlineAddress chainId={transaction.chainId} address={transaction.safe} disableLinks={disableLinks} /> on{" "}
				{chainName}
			</div>
			<div>
				{opString(transaction.operation)}{" "}
				<InlineAddress chainId={transaction.chainId} address={transaction.to} disableLinks={disableLinks} /> with{" "}
				{valueString(transaction.value, chainInfo?.nativeCurrency)} and {dataString(transaction.data)}
			</div>
		</>
	);
};

export const TransactionDataDetails = ({ data }: { data: Hex }) => {
	const [settings] = useSettings();
	return (
		<>
			<div className={"flex justify-between"}>
				<p className={"text-xs"}>Data ({dataString(data)})</p>
				<a className={"text-xs"} href={`${settings.decoder}${data}`} target="_blank" rel="noopener noreferrer">
					decode
				</a>
			</div>
			<p className={"break-all font-mono"}>{data}</p>
		</>
	);
};

export const NoTransactionProposalScreen = ({ transaction }: { transaction: SafeTransaction }) => {
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
};

export const TransactionProposals = ({ transaction }: { transaction: SafeTransaction }) => {
	const safeTxHash = useMemo(() => {
		return calculateSafeTxHash(transaction);
	}, [transaction]);
	const proposals = useProposalsForTransaction(safeTxHash);
	return (
		<div className={"space-y-4"}>
			<BoxTitle>Transaction Proposals</BoxTitle>
			{proposals.isLoading && <Box>Loading</Box>}
			{!proposals.isLoading && proposals.data.length === 0 && <NoTransactionProposalScreen transaction={transaction} />}
			{!proposals.isLoading &&
				proposals.data.map((proposal) => (
					<div key={`${proposal.transactionHash}:${proposal.epoch}`}>
						<Link to="/proposal" search={{ id: proposal.transactionHash }}>
							<Box className={"hover:bg-surface-hover"}>
								<TransactionProposalDetails proposal={proposal} disableLinks />
							</Box>
						</Link>
					</div>
				))}
		</div>
	);
};
