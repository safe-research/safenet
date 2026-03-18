import { cn } from "@/lib/utils";

function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
	return (
		<div
			className={cn("animate-pulse motion-reduce:animate-none rounded-card bg-surface-1/50", className)}
			{...props}
		/>
	);
}

export { Skeleton };
