import { cn } from "@/lib/utils";

function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
	return (
		<div
			className={cn(
				"relative overflow-hidden rounded-card bg-surface-outline",
				"after:content-[''] after:absolute after:inset-0 after:translate-x-[-100%]",
				"after:bg-gradient-to-r after:from-transparent after:via-white/40 after:to-transparent",
				"after:animate-shimmer motion-reduce:after:animate-none",
				className,
			)}
			{...props}
		/>
	);
}

export { Skeleton };
