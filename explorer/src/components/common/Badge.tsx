import { cn } from "@/lib/utils";

export function Badge({
	className,
	bgColor,
	fgColor,
	title,
	children,
}: {
	className?: string;
	bgColor?: string;
	fgColor?: string;
	title?: string;
	children: React.ReactNode;
}) {
	return (
		<span
			className={cn(
				"inline-block w-fit whitespace-nowrap px-1.5 py-0.5 text-2xs font-semibold leading-none rounded-full",
				className,
			)}
			style={bgColor !== undefined || fgColor !== undefined ? { backgroundColor: bgColor, color: fgColor } : undefined}
			title={title}
		>
			{children}
		</span>
	);
}
