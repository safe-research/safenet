import type { FieldError, FieldPath, FieldValues, UseFormRegister } from "react-hook-form";
import { Button } from "@/components/common/Button";
import { Input } from "@/components/common/Input";
import { Label } from "@/components/common/Label";
import { Spinner } from "@/components/common/Spinner";

function FormItem<T extends FieldValues>({
	id,
	error,
	label,
	placeholder,
	disabled,
	register,
	className,
}: {
	id: FieldPath<T>;
	error: FieldError | undefined;
	label: string;
	placeholder?: string;
	disabled?: boolean;
	register: UseFormRegister<T>;
	className?: string;
}) {
	return (
		<div className={className}>
			<Label htmlFor={id} className="mb-1">
				{label}
			</Label>
			<Input id={id} type="text" {...register(id)} placeholder={placeholder} disabled={disabled} className="mt-1" />
			{error && <p className="mt-1 text-sm text-error">{error.message}</p>}
		</div>
	);
}

function SubmitItem({
	isSubmitting,
	actionTitle,
	disabled = false,
	showProcessingText = true,
	className = "",
}: {
	isSubmitting: boolean;
	actionTitle: string;
	disabled?: boolean;
	className?: string;
	showProcessingText?: boolean;
}) {
	return (
		<div className={`pt-4 flex space-x-4 ${className}`}>
			<Button
				type="submit"
				variant="primary"
				disabled={isSubmitting || disabled}
				className="flex-1 flex justify-center items-center px-6 py-3 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{isSubmitting ? (
					<>
						<Spinner className="-ml-1 mr-3 text-surface-1" />
						{showProcessingText && "Processing..."}
					</>
				) : (
					actionTitle
				)}
			</Button>
		</div>
	);
}

function ErrorItem({ error }: { error: string | undefined }) {
	return (
		error && (
			<div className="mt-6 p-4 bg-error-surface border border-error-outline rounded-card">
				<h3 className="text-sm font-medium text-error">Error</h3>
				<p className="mt-1 text-sm text-error">{error}</p>
			</div>
		)
	);
}

export { FormItem, SubmitItem, ErrorItem };
