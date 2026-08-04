import { Fragment } from "react";
import type { Address } from "viem";
import { CopyButton } from "./CopyButton";
import { InfoPopover } from "./InfoPopover";

export function AnnotatedAddressList({
	accounts,
	active,
	label,
	popoverContent,
}: {
	accounts: Address[];
	active?: Address[];
	label: (address: Address, isActive: boolean) => string;
	popoverContent?: (address: Address) => React.ReactNode;
}) {
	const activeSet = new Set(active ?? accounts);
	const all = active ? Array.from(new Set([...accounts, ...active])) : accounts;

	const items = all
		.map((address) => ({ address, isActive: activeSet.has(address) }))
		.sort(
			(a, b) =>
				Number(b.isActive) - Number(a.isActive) ||
				label(a.address, a.isActive).localeCompare(label(b.address, b.isActive)),
		);

	return (
		<>
			{items.map((item, index) => (
				<Fragment key={item.address}>
					<InfoPopover
						trigger={
							<span className="cursor-pointer underline decoration-dotted">{label(item.address, item.isActive)}</span>
						}
					>
						<div className="flex items-center gap-1">
							<span className="font-mono text-xs">{item.address}</span>
							<CopyButton value={item.address} />
						</div>
						{popoverContent?.(item.address)}
					</InfoPopover>
					{index < items.length - 1 && ", "}
				</Fragment>
			))}
		</>
	);
}
