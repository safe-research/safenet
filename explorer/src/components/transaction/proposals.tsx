import { Link } from "@tanstack/react-router";
import { useMemo } from "react";
import { formatEther, type Hex, size } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { shortAddress } from "@/lib/address";
import type { TransactionProposal } from "@/lib/consensus";
import { calculateSafeTxHash } from "@/lib/safe";
import { Box } from "../Groups";

const opString = (operation: 0 | 1) => (operation === 0 ? "CALL" : "DELEGATECALL");
const valueString = (value: bigint) => `${formatEther(value)} ETH`;
const dataString = (data: Hex) => `${size(data)} bytes of data`;

export const TransactionProposalDetails = ({
	proposal,
	hideProposedAt,
}: {
	proposal: TransactionProposal;
	hideProposedAt?: boolean;
}) => {
	const safeTxHash = useMemo(() => {
		return calculateSafeTxHash(proposal);
	}, [proposal]);
	return (
		<>
			<div className={"flex justify-between"}>
				<p className={"text-xs"}>Safe Tx Hash: {safeTxHash}</p>
				{hideProposedAt !== true && <p className={"text-xs"}>{proposal.proposedAt}</p>}
			</div>
			<p>
				{shortAddress(proposal.transaction.account)} on {proposal.transaction.chainId}
			</p>
			<p>
				{opString(proposal.transaction.operation)} {shortAddress(proposal.transaction.to)} with{" "}
				{valueString(proposal.transaction.value)} and {dataString(proposal.transaction.data)}
			</p>
		</>
	);
};

export const TransactionProposalDataDetails = ({ proposal }: { proposal: TransactionProposal }) => {
	const [settings] = useSettings();
	return (
		<>
			<div className={"flex justify-between"}>
				<p className={"text-xs"}>Data ({dataString(proposal.transaction.data)})</p>
				<a className={"text-xs"} href={`${settings.decoder}${proposal.transaction.data}`} target="_blank">
					decode
				</a>
			</div>
			<p className={"break-all font-mono"}>{proposal.transaction.data}</p>
		</>
	);
};

export const TransactionProposalList = ({ proposals }: { proposals: TransactionProposal[] }) => {
	console.log({ proposals });
	return (
		<>
			<div className="w-full p-2 text-xs text-right">{proposals.length} recent proposals</div>
			<div className={"space-y-4"}>
				{proposals.map((proposal) => (
					<div key={proposal.message}>
						<Link to="/proposal" search={{ id: proposal.message }}>
							<Box className={"hover:bg-gray-100"}>
								<TransactionProposalDetails proposal={proposal} />
							</Box>
						</Link>
					</div>
				))}
			</div>
		</>
	);
};
