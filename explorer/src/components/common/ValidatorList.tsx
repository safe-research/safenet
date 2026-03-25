import { Fragment } from "react";
import type { Address } from "viem";
import { shortAddress } from "@/lib/address";
import type { ValidatorInfo } from "@/lib/validators/info";

export const createMapInfo =
	(validatorInfoMap: Map<Address, ValidatorInfo> | null | undefined) => (suffix: string) => (address: Address) =>
		`${validatorInfoMap?.get(address)?.label ?? shortAddress(address)} ${suffix}`;

export function ValidatorList({
	all,
	active,
	mapInfo,
	completed,
}: {
	all: Address[];
	active: Address[];
	mapInfo: (suffix: string) => (address: Address) => string;
	completed: boolean;
}) {
	const activeSet = new Set(active);

	const activeItems = active
		.map((address) => ({ address, label: mapInfo("✅")(address) }))
		.sort((a, b) => a.label.localeCompare(b.label));

	const inactiveItems = all
		.filter((address) => !activeSet.has(address))
		.map((address) => ({ address, label: mapInfo(completed ? "❌" : "⏳")(address) }))
		.sort((a, b) => a.label.localeCompare(b.label));

	const allItems = [...activeItems, ...inactiveItems];

	return (
		<>
			{allItems.map((item, index) => (
				<Fragment key={item.address}>
					<span title={item.address}>{item.label}</span>
					{index < allItems.length - 1 && ", "}
				</Fragment>
			))}
		</>
	);
}
