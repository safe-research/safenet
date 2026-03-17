import { CheckIcon, ClipboardDocumentIcon } from "@heroicons/react/24/outline";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/common/Button";

export const CopyButton = ({ value, className }: { value: string; className?: string }) => {
	const [copied, setCopied] = useState(false);
	const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

	useEffect(() => {
		return () => {
			if (timerRef.current !== null) {
				clearTimeout(timerRef.current);
			}
		};
	}, []);

	const handleCopy = async () => {
		await navigator.clipboard.writeText(value);
		if (timerRef.current !== null) {
			clearTimeout(timerRef.current);
		}
		setCopied(true);
		timerRef.current = setTimeout(() => setCopied(false), 2000);
	};

	return (
		<Button
			variant="icon"
			onClick={handleCopy}
			aria-label={copied ? "Copied" : "Copy to clipboard"}
			className={className}
		>
			{copied ? <CheckIcon className="h-4 w-4" /> : <ClipboardDocumentIcon className="h-4 w-4" />}
		</Button>
	);
};
