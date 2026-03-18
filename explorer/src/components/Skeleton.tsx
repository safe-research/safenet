import { cn } from "@/lib/utils";

function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
	return (
		<div
			className={cn(
				"animate-shimmer motion-reduce:animate-none rounded-card bg-gradient-to-r from-transparent via-surface-outline/40 to-transparent bg-[length:200%_100%]",
				className,
			)}
			{...props}
		/>
	);
}

export { Skeleton };
