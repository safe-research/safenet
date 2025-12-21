import { useMutation } from "@tanstack/react-query";
import { postTransactionProposal } from "@/lib/consensus";
import type { SafeTransaction } from "@/lib/safe/service";
import { useSettings } from "./useSettings";

export function useSubmitProposal() {
	const [settings] = useSettings();
	const relayer = settings.relayer;
	return {
		enabled: relayer !== undefined,
		mutation: useMutation({
			mutationFn: (transaction: SafeTransaction) =>
				relayer !== undefined ? postTransactionProposal(relayer, transaction) : Promise.reject("No relayer"),
		}),
	};
}
