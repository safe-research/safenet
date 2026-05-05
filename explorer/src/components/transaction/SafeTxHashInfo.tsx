import { useMemo } from "react";
import { InformationCircleIcon } from "@heroicons/react/24/outline";
import { CopyButton } from "@/components/common/CopyButton";
import { InfoPopover } from "@/components/common/InfoPopover";
import { InlineHash } from "@/components/common/InlineHash";
import type { SafeTransaction } from "@/lib/consensus";
import { calculateDomainHash, calculateMessageHash } from "@/lib/safe/hashing";

export function SafeTxHashInfo({ transaction }: { transaction: SafeTransaction }) {
	const domainHash = useMemo(
		() => calculateDomainHash(transaction.chainId, transaction.safe),
		[transaction.chainId, transaction.safe],
	);
	const messageHash = useMemo(() => calculateMessageHash(transaction), [transaction]);

	return (
		<InfoPopover trigger={<InformationCircleIcon className="h-4 w-4 text-muted cursor-pointer" />}>
			<div className="grid grid-cols-[max-content_auto] items-center gap-x-2 gap-y-1">
				<span className="text-muted">Domain Hash:</span>
				<div className="flex items-center gap-1 whitespace-nowrap">
					<InlineHash hash={domainHash} />
					<CopyButton value={domainHash} />
				</div>
				<span className="text-muted">Message Hash:</span>
				<div className="flex items-center gap-1 whitespace-nowrap">
					<InlineHash hash={messageHash} />
					<CopyButton value={messageHash} />
				</div>
			</div>
		</InfoPopover>
	);
}
