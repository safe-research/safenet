import { cn } from "@/lib/utils";

export function Badge({
	className,
	bgColor,
	fgColor,
	children,
}: {
	className?: string;
	bgColor?: string;
	fgColor?: string;
	children: React.ReactNode;
}) {
	return (
		<span
			className={cn("inline-block w-fit px-1.5 py-0.5 text-2xs font-semibold leading-none rounded-full", className)}
			style={bgColor !== undefined || fgColor !== undefined ? { backgroundColor: bgColor, color: fgColor } : undefined}
		>
			{children}
		</span>
	);
}
