import { standardSchemaResolver } from "@hookform/resolvers/standard-schema";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { ErrorItem, FormItem, SubmitItem } from "@/components/Forms";
import { useSettings } from "@/hooks/useSettings";
import { checkedAddressSchema } from "@/lib/schemas";
import { type Settings, updateSettings } from "@/lib/settings";

const emptyToUndefined = <T,>(pipe: z.ZodType<T, string>) =>
	z
		.string()
		.trim()
		.transform((v) => (v === "" ? undefined : v))
		.pipe(pipe.optional());

const settingsFormSchema = z.object({
	consensus: emptyToUndefined(checkedAddressSchema),
	decoder: emptyToUndefined(z.url()),
	rpc: emptyToUndefined(z.url()),
	relayer: emptyToUndefined(z.url()),
});

type SettingsFormInput = z.input<typeof settingsFormSchema>;

function SettingsForm({ onSubmitted }: { onSubmitted?: () => void }) {
	const [settings] = useSettings();
	const [isSubmitting, setIsSubmitting] = useState(false);
	const [error, setError] = useState<string>();
	const {
		register,
		handleSubmit,
		reset,
		formState: { errors, isDirty },
	} = useForm<SettingsFormInput, unknown, Partial<Settings>>({
		resolver: standardSchemaResolver(settingsFormSchema),
		defaultValues: settings,
	});

	const onSubmit = async (data: Partial<Settings>) => {
		setError(undefined);

		try {
			setIsSubmitting(true);
			updateSettings(data);
			reset(data);
			onSubmitted?.();
			console.log("Updated Settings!");
		} catch (err: unknown) {
			const message = err instanceof Error ? err.message : "An error occured";
			setError(message);
		} finally {
			setIsSubmitting(false);
		}
	};

	return (
		<form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
			<FormItem id="rpc" register={register} error={errors.rpc} label="RPC Url" />
			<FormItem id="decoder" register={register} error={errors.decoder} label="Decoder Url" />
			<FormItem id="relayer" register={register} error={errors.relayer} label="Relayer Url" />

			<FormItem
				id="consensus"
				register={register}
				error={errors.consensus}
				label="Consensus Address"
				placeholder="0x…"
			/>

			<SubmitItem actionTitle="Save" isSubmitting={isSubmitting} disabled={!isDirty} />

			<ErrorItem error={error} />
		</form>
	);
}

export { SettingsForm };
