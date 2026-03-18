export function formatLastUpdated(dataUpdatedAt: number): string {
	if (dataUpdatedAt === 0) return "—";
	return new Intl.DateTimeFormat(undefined, {
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
		timeZoneName: "short",
	}).format(new Date(dataUpdatedAt));
}

import { Spinner } from "@/components/common/Spinner";

export function TransactionListControls({
	isFetching,
	dataUpdatedAt,
	autoRefresh,
	onRefetch,
	onToggleAutoRefresh,
}: {
	isFetching: boolean;
	dataUpdatedAt: number;
	autoRefresh: boolean;
	onRefetch: () => void;
	onToggleAutoRefresh: () => void;
}) {
	return (
		<div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm py-2">
			<button
				type="button"
				onClick={onRefetch}
				disabled={isFetching}
				className="flex items-center gap-1 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{isFetching && <Spinner className="h-4 w-4" />}
				Refresh now
			</button>
			<span>
				Auto-refresh:{" "}
				<button
					type="button"
					onClick={onToggleAutoRefresh}
					aria-pressed={autoRefresh}
					className="cursor-pointer font-semibold"
				>
					{autoRefresh ? "ON" : "OFF"}
				</button>
			</span>
			<span>Last updated: {formatLastUpdated(dataUpdatedAt)}</span>
		</div>
	);
}
