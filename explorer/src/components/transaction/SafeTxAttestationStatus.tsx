import { AnnotatedAddressList } from "@/components/common/AnnotatedAddressList";
import { useAttestationStatus } from "@/hooks/useSigningProgress";
import { useValidatorInfoMap } from "@/hooks/useValidatorInfo";
import type { TransactionProposal } from "@/lib/consensus";
import { createStatusMapInfo, mapInfo } from "@/lib/validators/info";
import { Skeleton } from "../Skeleton";

export function SafeTxAttestationStatus({ proposal }: { proposal: TransactionProposal }) {
	const validatorInfo = useValidatorInfoMap();
	const status = useAttestationStatus(
		proposal.safeTxHash,
		proposal.epoch,
		proposal.proposedAt.block,
		proposal.attestedAt?.block ?? null,
		proposal.oracle,
	);
	const allValidatorIds = Array.from(validatorInfo.data?.keys() ?? []);
	const committedIds = status.data?.committed.map((s) => s.address) ?? [];
	const signedIds = status.data?.signed.map((s) => s.address) ?? [];
	return (
		<>
			{status.isFetching && status.data === null && <Skeleton className="w-full h-10" />}
			{status.data !== null && (
				<div key={status.data.sid}>
					<p>Validators:</p>
					{status.data.status !== "completed" && (
						<div className={"md:flex md:justify-between"}>
							<p className={"ml-4"}>Committed:</p>
							<div>
								<AnnotatedAddressList
									accounts={allValidatorIds}
									active={committedIds}
									label={createStatusMapInfo(validatorInfo.data, true)}
								/>
							</div>
						</div>
					)}
					<div className={"md:flex md:justify-between"}>
						<p className={"ml-4"}>Attested:</p>
						<div>
							<AnnotatedAddressList
								accounts={allValidatorIds}
								active={signedIds}
								label={createStatusMapInfo(validatorInfo.data, status.data.status === "completed")}
							/>
						</div>
					</div>
					{status.data.declined.length > 0 && (
						<div className={"md:flex md:justify-between"}>
							<p className={"ml-4"}>Declined:</p>
							<div>
								<AnnotatedAddressList
									accounts={status.data.declined.map((s) => s.address)}
									label={(address) => mapInfo(validatorInfo.data, "❌", address)}
								/>
							</div>
						</div>
					)}
				</div>
			)}
		</>
	);
}
