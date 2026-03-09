import { ValidatorList } from "@/components/common/ValidatorList";
import { Skeleton } from "@/components/Skeleton";
import { useValidatorInfoMap } from "@/hooks/useValidatorInfo";
import type { KeyGenStatus } from "@/lib/coordinator/keygen";

function statusLabel(status: KeyGenStatus): string {
	if (status.compromised) return "COMPROMISED";
	if (status.finalized) return "FINALIZED";
	if (status.confirmed.length > 0) return "CONFIRMING";
	if (status.shared.length > 0) return "SHARING";
	if (status.committed.length > 0) return "COMMITTING";
	return "STARTED";
}

function statusColor(status: KeyGenStatus): string {
	if (status.compromised) return "text-red-500";
	if (status.finalized) return "text-green-500";
	return "text-yellow-500";
}

export function KeyGenStatusItem({ status }: { status: KeyGenStatus | null; isLoading?: boolean }) {
	const validatorInfo = useValidatorInfoMap();

	if (status === null) {
		return <Skeleton className="w-full h-10 bg-primary/10" />;
	}

	const mapInfo = (suffix: string) => (identifier: bigint) =>
		`${validatorInfo?.data?.get(identifier)?.label ?? `Validator ${identifier}`} ${suffix}`;

	const terminal = status.finalized || status.compromised;
	const label = statusLabel(status);
	const allIds =
		status.committed.length > 0
			? status.committed.map((p) => p.identifier)
			: Array.from(validatorInfo.data?.keys() ?? []);

	return (
		<div className="bg-surface-0 border border-surface-outline rounded-md p-4 space-y-2 text-sm">
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
						<ValidatorList
							all={allIds}
							active={status.committed.map((p) => p.identifier)}
							mapInfo={mapInfo}
							completed={false}
						/>
					</p>
				</div>
			)}

			{!terminal && status.shared.length > 0 && (
				<div className="md:flex md:justify-between">
					<p className="ml-4">Shared:</p>
					<p>
						<ValidatorList
							all={allIds}
							active={status.shared.map((p) => p.identifier)}
							mapInfo={mapInfo}
							completed={false}
						/>
					</p>
				</div>
			)}

			{terminal && (
				<div className="md:flex md:justify-between">
					<p className="ml-4">Confirmed:</p>
					<p>
						<ValidatorList
							all={allIds}
							active={status.confirmed.map((p) => p.identifier)}
							mapInfo={mapInfo}
							completed={true}
						/>
					</p>
				</div>
			)}
		</div>
	);
}
