import type { ComponentPropsWithoutRef } from "react";
import { cn } from "@/lib/utils";

export function Label({ className, htmlFor, ...props }: ComponentPropsWithoutRef<"label"> & { htmlFor: string }) {
	// biome-ignore lint/a11y/noLabelWithoutControl: htmlFor is required by this component's type signature; Biome cannot infer non-empty string values at lint time
	return <label htmlFor={htmlFor} className={cn("block text-sm font-medium text-title", className)} {...props} />;
}
