import { useAttestationStatus } from "@/hooks/useSigningProgress";
import { useValidatorInfoMap } from "@/hooks/useValidatorInfo";
import type { TransactionProposal } from "@/lib/consensus";
import { Skeleton } from "../Skeleton";

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

export function SafeTxAttestationStatus({ proposal }: { proposal: TransactionProposal }) {
	const validatorInfo = useValidatorInfoMap();
	const status = useAttestationStatus(
		proposal.safeTxHash,
		proposal.epoch,
		proposal.proposedAt.block,
		proposal.attestedAt?.block ?? null,
	);
	const mapInfo = (suffix: string) => (identifier: bigint) =>
		`${validatorInfo?.data?.get(identifier)?.label ?? `Validator ${identifier}`} ${suffix}`;
	const allValidatorIds = Array.from(validatorInfo.data?.keys() ?? []);
	return (
		<>
			{status.isFetching && status.data === null && <Skeleton className="w-full h-10 bg-primary/10" />}
			{status.data !== null && (
				<div key={status.data.sid}>
					<p>Validators:</p>
					{!status.data.completed && (
						<div className={"md:flex md:justify-between"}>
							<p className={"ml-4"}>Committed:</p>
							<p>
								<ValidatorList
									all={allValidatorIds}
									active={status.data.committed.map((s) => s.identifier)}
									mapInfo={mapInfo}
									completed={status.data.completed}
								/>
							</p>
						</div>
					)}
					<div className={"md:flex md:justify-between"}>
						<p className={"ml-4"}>Attested:</p>
						<p>
							<ValidatorList
								all={allValidatorIds}
								active={status.data.signed.map((s) => s.identifier)}
								mapInfo={mapInfo}
								completed={status.data.completed}
							/>
						</p>
					</div>
				</div>
			)}
		</>
	);
}
