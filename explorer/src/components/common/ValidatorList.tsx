import type { ValidatorInfo } from "@/lib/validators/info";

export const createMapInfo =
	(validatorInfoMap: Map<bigint, ValidatorInfo> | null | undefined) => (suffix: string) => (identifier: bigint) =>
		`${validatorInfoMap?.get(identifier)?.label ?? `Validator ${identifier}`} ${suffix}`;

export function ValidatorList({
	all,
	active,
	mapInfo,
	completed,
}: {
	all: bigint[];
	active: bigint[];
	mapInfo: (suffix: string) => (identifier: bigint) => string;
	completed: boolean;
}) {
	return (
		<>
			{active
				.map(mapInfo("✅"))
				.sort()
				.concat(
					all
						.filter((v) => !active.includes(v))
						.map(mapInfo(completed ? "❌" : "⏳"))
						.sort(),
				)
				.join(", ")}
		</>
	);
}
