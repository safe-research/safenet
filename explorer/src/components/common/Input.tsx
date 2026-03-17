import type { ComponentPropsWithoutRef } from "react";
import { cn } from "@/lib/utils";

export function Input({ className, ...props }: ComponentPropsWithoutRef<"input">) {
	return (
		<input
			className={cn(
				"block w-full border border-surface-outline bg-surface-1 text-title rounded-input px-3 py-2",
				className,
			)}
			{...props}
		/>
	);
}
