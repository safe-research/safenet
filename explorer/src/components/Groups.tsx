import { cn } from "@/lib/utils";

interface GroupProps {
	/** The content to display within the button. */
	children: React.ReactNode;
	/** Optional additional CSS classes. */
	className?: string;
}

function Box({ children, className = "" }: GroupProps) {
	return <div className={cn("bg-surface-1 p-6 border border-surface-outline rounded-lg", className)}>{children}</div>;
}

function BoxTitle({ children }: { children: React.ReactNode }) {
	return <h2 className="text-lg font-semibold text-title mb-4">{children}</h2>;
}

function Container({ children, className = "" }: GroupProps) {
	return (
		<div className="bg-surface-0 h-full max-h-full">
			<div className={cn("max-w-4xl mx-auto px-4 py-12 sm:px-6 lg:px-8", className)}>{children}</div>
		</div>
	);
}

function ContainerTitle({ children }: { children: React.ReactNode }) {
	return <h1 className="text-3xl font-bold text-title mb-4">{children}</h1>;
}

function ContainerSectionTitle({ children }: { children: React.ReactNode }) {
	return <h1 className="text-3xl font-bold text-title">{children}</h1>;
}

export { Box, BoxTitle, Container, ContainerTitle, ContainerSectionTitle };
