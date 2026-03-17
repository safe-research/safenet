import { cn } from "@/lib/utils";

const variantClasses = {
	positive: "bg-positive text-positive-foreground",
	pending: "bg-pending text-pending-foreground",
	error: "bg-error text-error-foreground",
	warning: "bg-warning-surface text-warning border border-warning-outline",
	neutral: "bg-surface-0 text-muted border border-surface-outline",
} as const;

type BadgeVariant = keyof typeof variantClasses;

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
				"inline-flex items-center w-fit whitespace-nowrap px-1.5 py-0.5 text-2xs font-semibold leading-none rounded-full",
				variant && variantClasses[variant],
				className,
			)}
			style={bgColor !== undefined || fgColor !== undefined ? { backgroundColor: bgColor, color: fgColor } : undefined}
			title={title}
		>
			{children}
		</span>
	);
}
