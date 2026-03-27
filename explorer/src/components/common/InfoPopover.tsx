import { useEffect, useRef, useState } from "react";

export function InfoPopover({ trigger, children }: { trigger: React.ReactNode; children: React.ReactNode }) {
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLSpanElement>(null);

	useEffect(() => {
		if (!open) return;
		const handler = (e: MouseEvent) => {
			if (!ref.current?.contains(e.target as Node)) setOpen(false);
		};
		document.addEventListener("mousedown", handler);
		return () => document.removeEventListener("mousedown", handler);
	}, [open]);

	return (
		<span className="relative inline-block" ref={ref}>
			<button type="button" onClick={() => setOpen((v) => !v)} className="cursor-pointer">
				{trigger}
			</button>
			{open && (
				<div className="absolute left-0 top-full z-10 mt-1 min-w-48 rounded border border-surface-outline bg-surface-1 p-2 shadow-md text-sm space-y-1">
					{children}
				</div>
			)}
		</span>
	);
}
