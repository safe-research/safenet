import { useQuery } from "@tanstack/react-query";
import { useSettings } from "@/hooks/useSettings";
import { loadValidatorInfoMap, type ValidatorInfo } from "@/lib/validators/info";

export function useValidatorInfoMap() {
	const [settings] = useSettings();
	return useQuery<Map<bigint, ValidatorInfo> | null, Error>({
		queryKey: ["validatorInfoMap", settings.validatorInfo],
		queryFn: () => loadValidatorInfoMap(settings.validatorInfo),
		initialData: null,
	});
}
