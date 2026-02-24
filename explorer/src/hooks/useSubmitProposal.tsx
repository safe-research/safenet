import { useMutation } from "@tanstack/react-query";
import { useSettings } from "@/hooks/useSettings";
import { postTransactionProposal, type SafeTransaction } from "@/lib/consensus";

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
