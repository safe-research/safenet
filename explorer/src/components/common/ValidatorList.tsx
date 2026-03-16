import type { Address } from "viem";
import type { ValidatorInfo } from "@/lib/validators/info";

export const createMapInfo =
	(validatorInfoMap: Map<Address, ValidatorInfo> | null | undefined) => (suffix: string) => (address: Address) =>
		`${validatorInfoMap?.get(address)?.label ?? `${address.slice(0, 6)}…${address.slice(-4)}`} ${suffix}`;

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
	return (
		<>
			{active
				.map(mapInfo("✅"))
				.sort()
				.concat(
					all
						.filter((v) => !activeSet.has(v))
						.map(mapInfo(completed ? "❌" : "⏳"))
						.sort(),
				)
				.join(", ")}
		</>
	);
}
