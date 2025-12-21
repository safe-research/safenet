import { Link } from "@tanstack/react-router";
import { useMemo } from "react";
import type { Hex } from "viem";
import { usePorposalsForTransaction } from "@/hooks/useProposalsForTransaction";
import { useSettings } from "@/hooks/useSettings";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { MetaTransaction } from "@/lib/consensus";
import { dataString, opString, valueString } from "@/lib/safe/formatting";
import { metaTxHash } from "@/lib/safe/hashing";
import type { SafeTransaction } from "@/lib/safe/service";
import { InlineAddress } from "../common/InlineAddress";
import { Box, BoxTitle } from "../Groups";
import { TransactionProposalDetails } from "./Proposals";

export const SafeTxOverview = ({
	title,
	transaction,
	timestamp,
}: {
	title: string;
	transaction: MetaTransaction;
	timestamp?: string;
}) => {
	const chainIdString = `${transaction.chainId}`;
	const chainName = SAFE_SERVICE_CHAINS[chainIdString]?.name ?? chainIdString;
	return (
		<>
			<div className={"flex justify-between"}>
				<p className={"text-xs"}>{title}</p>
				{timestamp !== undefined && <p className={"text-xs"}>{timestamp}</p>}
			</div>
			<p>
				<InlineAddress chainId={transaction.chainId} address={transaction.account} /> on {chainName}
			</p>
			<p>
				{opString(transaction.operation)} <InlineAddress chainId={transaction.chainId} address={transaction.to} /> with{" "}
				{valueString(transaction.value)} and {dataString(transaction.data)}
			</p>
		</>
	);
};

export const TransactionDataDetails = ({ data }: { data: Hex }) => {
	const [settings] = useSettings();
	return (
		<>
			<div className={"flex justify-between"}>
				<p className={"text-xs"}>Data ({dataString(data)})</p>
				<a className={"text-xs"} href={`${settings.decoder}${data}`} target="_blank">
					decode
				</a>
			</div>
			<p className={"break-all font-mono"}>{data}</p>
		</>
	);
};

export const TransactionProposals = ({ transaction }: { transaction: SafeTransaction }) => {
	const proposalTxHash = useMemo(() => {
		return metaTxHash(transaction);
	}, [transaction]);
	const proposals = usePorposalsForTransaction(proposalTxHash);
	return (
		<div className={"space-y-4"}>
			<BoxTitle>Transaction Proposals</BoxTitle>
			{proposals.isLoading && <Box>Loading</Box>}
			{!proposals.isLoading && proposals.data.length === 0 && <Box>No proposals for this transaction!</Box>}
			{!proposals.isLoading &&
				proposals.data.map((proposal) => (
					<div key={proposal.message}>
						<Link to="/proposal" search={{ id: proposal.message }}>
							<Box className={"hover:bg-surface-hover"}>
								<TransactionProposalDetails proposal={proposal} />
							</Box>
						</Link>
					</div>
				))}
		</div>
	);
};
