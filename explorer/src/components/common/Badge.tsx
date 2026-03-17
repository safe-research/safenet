import { cn } from "@/lib/utils";

type BadgeVariant = "positive" | "pending" | "error" | "warning" | "neutral";

const variantClasses: Record<BadgeVariant, string> = {
	positive: "bg-positive text-positive-foreground",
	pending: "bg-pending text-pending-foreground",
	error: "bg-error-surface text-error border border-error-outline",
	warning: "bg-warning-surface text-warning border border-warning-outline",
	neutral: "bg-surface-0 text-muted border border-surface-outline",
};

export function Badge({
	className,
	bgColor,
	fgColor,
	title,
	variant,
	children,
}: {
	className?: string;
	bgColor?: string;
	fgColor?: string;
	title?: string;
	variant?: BadgeVariant;
	children: React.ReactNode;
}) {
	return (
		<span
			className={cn(
				"inline-block w-fit px-1.5 py-0.5 text-2xs font-semibold leading-none rounded-full",
				variant !== undefined ? variantClasses[variant] : undefined,
				className,
			)}
			style={bgColor !== undefined || fgColor !== undefined ? { backgroundColor: bgColor, color: fgColor } : undefined}
			title={title}
		>
			{children}
		</span>
	);
}
