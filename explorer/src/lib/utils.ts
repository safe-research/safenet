import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export function jsonReplacer(_key: string, value: unknown): unknown {
	if (typeof value === "bigint") {
		return value.toString();
	}
	return value;
}
