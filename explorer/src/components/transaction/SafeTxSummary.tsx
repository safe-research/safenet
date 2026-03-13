import { ArrowTopRightOnSquareIcon } from "@heroicons/react/24/solid";
import { useState } from "react";
import { Badge } from "@/components/common/Badge";
import { CopyButton } from "@/components/common/CopyButton";
import { InlineAddress } from "@/components/common/InlineAddress";
import { BoxTitle } from "@/components/Groups";
import { useSettings } from "@/hooks/useSettings";
import type { SafeTransaction } from "@/lib/consensus";
import { dataString, opString, valueString } from "@/lib/safe/formatting";

const DATA_LIMIT = 206;

export function SafeTxSummary({ transaction }: { transaction: SafeTransaction }) {
	const [showAll, setShowAll] = useState(false);
	const [settings] = useSettings();
	const { data, operation, to, value, chainId } = transaction;
	const canShowMore = data.length > DATA_LIMIT;

	return (
		<div className="space-y-2">
			<BoxTitle>Transaction Summary</BoxTitle>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Operation:</span>
				<Badge className={operation === 1 ? "bg-warning-surface text-warning" : "bg-surface-outline text-title"}>
					{opString(operation)}
				</Badge>
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">To:</span>
				<InlineAddress chainId={chainId} address={to} />
				<CopyButton value={to} />
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Value:</span>
				<span className="text-sm">{valueString(value)}</span>
			</div>
			<div className="flex flex-wrap items-center gap-2">
				<span className="text-sm font-medium">Calldata:</span>
				<span className="text-sm">{dataString(data)}</span>
			</div>
			<div>
				<p className="text-sm font-medium mb-1">Raw calldata:</p>
				<p className="break-all font-mono text-sm">
					{showAll || !canShowMore ? data : `${data.slice(0, DATA_LIMIT - 3)}…`}
					{canShowMore && (
						<button type="button" className="cursor-pointer ml-2" onClick={() => setShowAll(!showAll)}>
							{showAll ? "less" : "more"}
						</button>
					)}
				</p>
				<div className="flex items-center gap-2 mt-2">
					<CopyButton value={data} />
					<a className="text-xs" href={`${settings.decoder}${data}`} target="_blank" rel="noopener noreferrer">
						Decode <ArrowTopRightOnSquareIcon className="inline-block h-3 w-3 mb-1" />
					</a>
				</div>
			</div>
		</div>
	);
}
