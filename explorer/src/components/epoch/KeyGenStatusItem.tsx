import type { Address } from "viem";
import { AnnotatedAddressList } from "@/components/common/AnnotatedAddressList";
import { Skeleton } from "@/components/Skeleton";
import { useValidatorInfoMap } from "@/hooks/useValidatorInfo";
import type { KeyGenStatus } from "@/lib/coordinator/keygen";
import { createStatusMapInfo } from "@/lib/validators/info";

function statusLabel(status: KeyGenStatus): string {
	if (status.compromised) return "COMPROMISED";
	if (status.finalized) return "FINALIZED";
	if (status.confirmed.length > 0) return "CONFIRMING";
	if (status.shared.length > 0) return "SHARING";
	if (status.committed.length > 0) return "COMMITTING";
	return "STARTED";
}

function statusColor(status: KeyGenStatus): string {
	if (status.compromised) return "text-error";
	if (status.finalized) return "text-positive";
	return "text-pending";
}

export function KeyGenStatusItem({ status }: { status: KeyGenStatus | null }) {
	const validatorInfo = useValidatorInfoMap();

	if (status === null) {
		return <Skeleton className="w-full h-10 bg-primary/10" />;
	}

	const terminal = status.finalized || status.compromised;
	const label = statusLabel(status);
	const allIds: Address[] = Array.from(validatorInfo.data?.keys() ?? []);
	const committedIds = status.committed.map((p) => p.address);
	const sharedIds = status.shared.map((p) => p.address);
	const confirmedIds = status.confirmed.map((p) => p.address);

	return (
		<div className="bg-surface-0 border border-surface-outline rounded-card p-4 space-y-2 text-sm">
			<div className="flex items-center gap-2">
				<span className={`font-semibold ${statusColor(status)}`}>[{label}]</span>
				<span className="font-mono text-muted">KeyGen {status.gid.slice(0, 18)}…</span>
			</div>
			<p>
				Threshold: {status.threshold} of {status.count}
			</p>

			{!terminal && status.committed.length > 0 && (
				<div className="md:flex md:justify-between">
					<p className="ml-4">Committed:</p>
					<p>
						<AnnotatedAddressList
							accounts={allIds}
							active={committedIds}
							label={createStatusMapInfo(validatorInfo.data, false)}
						/>
					</p>
				</div>
			)}

			{!terminal && status.shared.length > 0 && (
				<div className="md:flex md:justify-between">
					<p className="ml-4">Shared:</p>
					<p>
						<AnnotatedAddressList
							accounts={allIds}
							active={sharedIds}
							label={createStatusMapInfo(validatorInfo.data, false)}
						/>
					</p>
				</div>
			)}

			<div className="md:flex md:justify-between">
				<p className="ml-4">Confirmed:</p>
				<p>
					<AnnotatedAddressList
						accounts={allIds}
						active={confirmedIds}
						label={createStatusMapInfo(validatorInfo.data, terminal)}
					/>
				</p>
			</div>
		</div>
	);
}
