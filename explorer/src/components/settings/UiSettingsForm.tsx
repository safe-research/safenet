import { standardSchemaResolver } from "@hookform/resolvers/standard-schema";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { ErrorItem, SubmitItem } from "@/components/Forms";
import { useUiSettings } from "@/hooks/useSettings";
import { emptyToUndefined } from "@/lib/schemas";
import { type UiSettings, updateUiSettings } from "@/lib/settings";

const settingsFormSchema = z.object({
	theme: emptyToUndefined(z.union([z.literal("light"), z.literal("dark")])).optional(),
});

type SettingsFormInput = z.input<typeof settingsFormSchema>;

function UiSettingsForm({ onSubmitted }: { onSubmitted?: () => void }) {
	const [settings] = useUiSettings();
	const [isSubmitting, setIsSubmitting] = useState(false);
	const [error, setError] = useState<string>();
	const {
		register,
		handleSubmit,
		reset,
		formState: { errors, isDirty },
	} = useForm<SettingsFormInput, unknown, Partial<UiSettings>>({
		resolver: standardSchemaResolver(settingsFormSchema),
		defaultValues: settings,
	});

	const onSubmit = async (data: Partial<UiSettings>) => {
		setError(undefined);
		try {
			setIsSubmitting(true);
			updateUiSettings(data);
			reset(data);
			onSubmitted?.();
		} catch (err: unknown) {
			const message = err instanceof Error ? err.message : "An error occurred";
			setError(message);
		} finally {
			setIsSubmitting(false);
		}
	};

	return (
		<form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
			<div className="flex items-center grow justify-between">
				<label htmlFor="theme" className="font-medium text-title">
					Theme
				</label>
				<select id="theme" {...register("theme")} className="block bg-surface-1">
					<option value="">System Default</option>
					<option value="dark">Dark Theme</option>
					<option value="light">Light Theme</option>
				</select>
			</div>
			{errors.theme && <p className="mt-1 text-sm text-error">{errors.theme.message}</p>}

			<SubmitItem actionTitle="Save" isSubmitting={isSubmitting} disabled={!isDirty} />

			<ErrorItem error={error} />
		</form>
	);
}

export { UiSettingsForm };
