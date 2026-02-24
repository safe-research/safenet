import { useQuery } from "@tanstack/react-query";
import { loadValidatorInfoMap, type ValidatorInfo } from "@/lib/validators/info";

export function useValidatorInfoMap() {
	return useQuery<Map<bigint, ValidatorInfo> | null, Error>({
		queryKey: ["validatorInfoMap"],
		queryFn: () => loadValidatorInfoMap(),
		initialData: null,
	});
}
