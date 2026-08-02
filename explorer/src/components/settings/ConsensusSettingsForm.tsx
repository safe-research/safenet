import { standardSchemaResolver } from "@hookform/resolvers/standard-schema";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { ErrorItem, FormItem, SubmitItem } from "@/components/Forms";
import { useSettings } from "@/hooks/useSettings";
import { checkedAddressSchema, emptyToUndefined } from "@/lib/schemas";
import { type Settings, updateSettings } from "@/lib/settings";

const numberOrStringAsNumber = z
	.union([z.string(), z.number()])
	.transform((val) => (val === "" ? undefined : Number(val)));

// Accepts the already-parsed Address[] from Settings (used as react-hook-form's
// defaultValues) or the raw comma-separated text the user edits it as. Validation
// issues are reported on the field itself (not per-address) so FormItem can render them.
const addressListOrStringAsAddressList = z.union([z.array(checkedAddressSchema), z.string()]).transform((val, ctx) => {
	const addresses = (Array.isArray(val) ? val : val.split(","))
		.map((address) => address.trim())
		.filter((address) => address !== "");
	const parsed = z.array(checkedAddressSchema).safeParse(addresses);
	if (!parsed.success) {
		ctx.addIssue({ code: "custom", message: parsed.error.issues[0]?.message ?? "Invalid oracle address" });
		return z.NEVER;
	}
	return parsed.data;
});

const settingsFormSchema = z.object({
	consensus: emptyToUndefined(checkedAddressSchema),
	decoder: emptyToUndefined(z.url()),
	rpc: emptyToUndefined(z.url()),
	relayer: emptyToUndefined(z.url()),
	maxBlockRange: numberOrStringAsNumber.pipe(z.number().int().positive().optional()),
	validatorInfo: emptyToUndefined(z.url()),
	refetchInterval: numberOrStringAsNumber.pipe(z.number().int().nonnegative().optional()),
	blocksPerEpoch: numberOrStringAsNumber.pipe(z.number().int().positive().optional()),
	signingTimeout: numberOrStringAsNumber.pipe(z.number().int().positive().optional()),
	oracles: addressListOrStringAsAddressList.optional(),
});

type SettingsFormInput = z.input<typeof settingsFormSchema>;

function ConsensusSettingsForm({ onSubmitted }: { onSubmitted?: () => void }) {
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
		} catch (err: unknown) {
			const message = err instanceof Error ? err.message : "An error occurred";
			setError(message);
		} finally {
			setIsSubmitting(false);
		}
	};

	return (
		<form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
			<FormItem id="rpc" register={register} error={errors.rpc} label="RPC Url" />
			<FormItem id="maxBlockRange" register={register} error={errors.maxBlockRange} label="Max Block Range" />
			<FormItem id="decoder" register={register} error={errors.decoder} label="Decoder Url" />
			<FormItem id="relayer" register={register} error={errors.relayer} label="Relayer Url" />

			<FormItem
				id="consensus"
				register={register}
				error={errors.consensus}
				label="Consensus Address"
				placeholder="0x…"
			/>

			<FormItem id="blocksPerEpoch" register={register} error={errors.blocksPerEpoch} label="Blocks Per Epoch" />
			<FormItem
				id="signingTimeout"
				register={register}
				error={errors.signingTimeout}
				label="Signing Timeout (blocks)"
			/>
			<FormItem id="validatorInfo" register={register} error={errors.validatorInfo} label="Validator Info Url" />

			<FormItem
				id="oracles"
				register={register}
				error={errors.oracles}
				label="Oracle Addresses (comma-separated)"
				placeholder="0x…, 0x…"
			/>

			<FormItem
				id="refetchInterval"
				register={register}
				error={errors.refetchInterval}
				label="Refetch Interval (0 to disable refetching)"
			/>

			<SubmitItem actionTitle="Save" isSubmitting={isSubmitting} disabled={!isDirty} />

			<ErrorItem error={error} />
		</form>
	);
}

export { ConsensusSettingsForm };
