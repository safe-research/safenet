import { ArrowTopRightOnSquareIcon } from "@heroicons/react/24/solid";
import { useState } from "react";
import type { Hex } from "viem";
import { CopyButton } from "@/components/common/CopyButton";
import { useSettings } from "@/hooks/useSettings";
import { dataString } from "@/lib/safe/formatting";

const DATA_LIMIT = 206;

export function SafeTxDataDetails({ data }: { data: Hex }) {
	const [showAll, setShowAll] = useState(false);
	const [settings] = useSettings();
	const canShowMore = data.length > DATA_LIMIT;
	return (
		<>
			<div className={"flex justify-between"}>
				<p className={"text-xs"}>Data ({dataString(data)})</p>
				<div className="flex items-center gap-2">
					<CopyButton value={data} />
					<a className={"text-xs"} href={`${settings.decoder}${data}`} target="_blank" rel="noopener noreferrer">
						decode <ArrowTopRightOnSquareIcon className="inline-block h-3 w-3 mb-1" />
					</a>
				</div>
			</div>
			<p className={"break-all font-mono"}>
				{showAll || !canShowMore ? data : `${data.slice(0, DATA_LIMIT - 3)}…`}
				{canShowMore && (
					<button type="button" className={"cursor-pointer ml-2"} onClick={() => setShowAll(!showAll)}>
						{showAll ? "less" : "more"}
					</button>
				)}
			</p>
		</>
	);
}
