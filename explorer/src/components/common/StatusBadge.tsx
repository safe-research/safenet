export function StatusBadge({ attested }: { attested: boolean }) {
	if (attested) {
		return (
			<span className="inline-block px-1.5 py-0.5 text-[10px] font-semibold leading-none rounded border border-positive text-positive">
				ATTESTED
			</span>
		);
	}
	return (
		<span className="inline-block px-1.5 py-0.5 text-[10px] font-semibold leading-none rounded border border-pending text-pending">
			PROPOSED
		</span>
	);
}
