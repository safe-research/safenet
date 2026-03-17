import type { ComponentPropsWithoutRef } from "react";
import { cn } from "@/lib/utils";

export function Label({ className, ...props }: ComponentPropsWithoutRef<"label">) {
	// biome-ignore lint/a11y/noLabelWithoutControl: htmlFor is forwarded via props spread
	return <label className={cn("block text-sm font-medium text-title", className)} {...props} />;
}
