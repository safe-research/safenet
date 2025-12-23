import { Cog6ToothIcon } from "@heroicons/react/24/solid";
import { Link } from "@tanstack/react-router";
import { useConsensusState } from "@/hooks/useConsensusState";
import { SafeResearchBanner } from "./SafeResearch";

export default function Header() {
	const state = useConsensusState();
	return (
		// Header is a flex-col with 2 elements.
		// Element 1 is a flex-row with the Harbour name on the left and the wallet connection button / info on the right.
		// Element 2 is the Safe Research banner.
		<header className="sticky top-0 z-50 w-full flex flex-col items-center px-2 py-2 bg-surface-1 border-b border-surface-outline">
			<nav className="flex flex-row w-full justify-between mb-2">
				<Link to="/" className="text-xl font-semibold text-title hover:opacity-75 transition" search={{}}>
					Shieldnet
				</Link>
				<div className="flex items-center gap-2">
					Block: {state.data.currentBlock} | Epoch: {state.data.currentEpoch} | GroupId:{" "}
					{state.data.currentGroupId.slice(0, 10)}
					<Link to="/settings">
						<Cog6ToothIcon className="size-8 p-1 hover:opacity-40 transition-opacity duration-300 cursor-pointer" />
					</Link>
				</div>
			</nav>
			<SafeResearchBanner />
		</header>
	);
}
