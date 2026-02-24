import { InlineAddress } from "@/components/common/InlineAddress";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import type { SafeTransaction } from "@/lib/consensus";
import { dataString, opString, valueString } from "@/lib/safe/formatting";

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
				<p className={"text-xs break-all"}>{title}</p>
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
