import type { ToPathOption } from "@tanstack/react-router";
import { Link, useCanGoBack, useRouter } from "@tanstack/react-router";
import { cn } from "@/lib/utils";

interface BackButtonProps {
	/** The path to link to. Should be a registered route. */
	to: ToPathOption;
	/** Search parameters for the link. */
	search?: Record<string, string | number | bigint>;
	/** The content to display within the button. */
	children: React.ReactNode;
	/** Optional additional CSS classes. */
	className?: string;
}

/**
 * A generic button component that links to a previous page or a specified path.
 * Prepends a left arrow (←) to the children.
 * @param {BackButtonProps} props - The component props.
 * @returns JSX element representing the back button.
 */
function BackButton({ to, search, children, className = "" }: BackButtonProps) {
	return (
		<Link
			to={to}
			search={search}
			className={cn("cursor-pointer inline-flex items-center text-sub-title hover:underline", className)}
		>
			← {children}
		</Link>
	);
}

/**
 * A button component that pops to the previous page or if it cannot go back it will be hidden.
 * Uses (← Back) as an action text.
 * @param {BackButtonProps} props - The component props.
 * @returns JSX element representing the back button.
 */
function ConditionalBackButton({ className = "" }: { className?: string }) {
	const router = useRouter();
	const canGoBack = useCanGoBack();
	return canGoBack ? (
		<button
			type="button"
			onClick={() => router.history.back()}
			className={cn("cursor-pointer inline-flex items-center text-sub-title hover:underline", className)}
		>
			← Back
		</button>
	) : null;
}

export { BackButton, ConditionalBackButton };
