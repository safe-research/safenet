import { cn } from "@/lib/utils";

export function Badge({ className, children }: { className?: string; children: React.ReactNode }) {
	return (
		<span className={cn("inline-block px-1.5 py-0.5 text-[10px] font-semibold leading-none rounded border", className)}>
			{children}
		</span>
	);
}
