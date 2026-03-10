import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

export function Badge({
	className,
	style,
	children,
}: {
	className?: string;
	style?: CSSProperties;
	children: React.ReactNode;
}) {
	return (
		<span
			className={cn("inline-block w-fit px-1.5 py-0.5 text-[10px] font-semibold leading-none rounded-full", className)}
			style={style}
		>
			{children}
		</span>
	);
}
