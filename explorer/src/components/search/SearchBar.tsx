import { MagnifyingGlassIcon } from "@heroicons/react/24/solid";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { getAddress, isAddress, isHex, size } from "viem";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import { cn } from "@/lib/utils";

export function SearchBar({
	className,
	selectedNetwork,
	onSelectNetwork,
}: {
	className?: string;
	selectedNetwork?: string;
	onSelectNetwork: (id: string) => void;
}) {
	const navigate = useNavigate();
	const networks = Object.values(SAFE_SERVICE_CHAINS);
	const [idInput, setIdInput] = useState("");
	const [error, setError] = useState<string | null>(null);
	const handleSelected = () => {
		const cleanInput = idInput.trim();
		if (cleanInput.length === 0) return;
		if (isAddress(cleanInput)) {
			setError(null);
			navigate({
				to: "/safe",
				search: {
					safeAddress: getAddress(cleanInput),
					chainId: selectedNetwork,
				},
			});
		} else if (isHex(cleanInput) && size(cleanInput) === 32) {
			setError(null);
			navigate({
				to: "/safeTx",
				search: {
					chainId: selectedNetwork,
					safeTxHash: cleanInput,
				},
			});
		} else {
			setError("Invalid input: enter a Safe address or a 32-byte transaction hash.");
		}
	};
	return (
		<div className={cn("flex flex-col items-center w-full", className)}>
			<div className="flex w-[80%] justify-between border rounded-full px-2 py-1 items-center">
				<select
					className="block text-xs bg-surface-0 text-sm p-2"
					value={selectedNetwork ?? networks[0].id.toString()}
					onChange={(e) => onSelectNetwork(e.target.value)}
				>
					{networks.map((info) => (
						<option key={info.name} value={info.id.toString()}>
							{info.name}
						</option>
					))}
				</select>
				<input
					className="p-2 flex-1 text-xs"
					id="id"
					type="text"
					placeholder="0x..."
					value={idInput}
					onChange={(e) => {
						setIdInput(e.target.value);
						setError(null);
					}}
					onKeyDown={(e) => {
						if (e.key === "Enter") handleSelected();
					}}
				/>
				<MagnifyingGlassIcon
					onClick={() => handleSelected()}
					className="size-8 p-1 hover:opacity-40 transition-opacity duration-300 cursor-pointer"
				/>
			</div>
			{error !== null && <p className="w-[80%] mt-1 text-xs text-error">{error}</p>}
		</div>
	);
}
