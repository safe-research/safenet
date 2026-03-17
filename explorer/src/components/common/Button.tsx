import type { ComponentPropsWithoutRef } from "react";
import { cn } from "@/lib/utils";

const variantClasses = {
	primary: "bg-button hover:bg-button-hover text-button-content rounded-input px-4 py-2",
	ghost: "text-sub-title hover:text-title hover:underline",
	icon: "inline-flex items-center text-xs px-1.5 py-0.5 border border-surface-outline rounded-input hover:bg-surface-1 transition-colors cursor-pointer",
} as const;

type ButtonVariant = keyof typeof variantClasses;

export function Button({
	variant = "primary",
	className,
	...props
}: ComponentPropsWithoutRef<"button"> & { variant?: ButtonVariant }) {
	return <button type="button" className={cn(variantClasses[variant], className)} {...props} />;
}
