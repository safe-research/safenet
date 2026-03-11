import { CheckIcon, ClipboardDocumentIcon } from "@heroicons/react/24/outline";
import { useState } from "react";

export const CopyButton = ({ value }: { value: string }) => {
	const [copied, setCopied] = useState(false);

	const handleCopy = async () => {
		await navigator.clipboard.writeText(value);
		setCopied(true);
		setTimeout(() => setCopied(false), 2000);
	};

	return (
		<button
			type="button"
			onClick={handleCopy}
			aria-label={copied ? "Copied" : "Copy to clipboard"}
			className="inline-flex items-center text-xs px-1.5 py-0.5 rounded border border-surface-outline hover:bg-surface-1 transition-colors"
		>
			{copied ? <CheckIcon className="h-4 w-4" /> : <ClipboardDocumentIcon className="h-4 w-4" />}
		</button>
	);
};
