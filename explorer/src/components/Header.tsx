import { Link } from "@tanstack/react-router";
import { SafenetBetaLogo } from "@/components/common/SafenetBetaLogo";
import { useConsensusState } from "@/hooks/useConsensusState";

export default function Header() {
	const state = useConsensusState();
	return (
		<header className="sticky top-0 z-50 w-full flex flex-col gap-1 px-4 py-2 bg-surface-1 border-b border-surface-outline">
			{/* Primary nav row: logo + links + docs */}
			<div className="flex items-center justify-between w-full gap-4">
				<div className="flex items-center gap-4">
					<Link to="/" className="hover:opacity-75 transition shrink-0" search={{}}>
						<SafenetBetaLogo />
					</Link>
					<Link to="/" className="text-sm text-muted hover:text-title transition-colors" search={{}}>
						Explore
					</Link>
					<Link to="/settings" className="text-sm text-muted hover:text-title transition-colors">
						Settings
					</Link>
				</div>
				<a
					href={__DOCS_URL__}
					target="_blank"
					rel="noopener noreferrer"
					className="text-sm text-muted hover:text-title transition-colors whitespace-nowrap"
				>
					Docs ↗
				</a>
			</div>

			{/* Status row: block / epoch / group id — always below on all screen sizes */}
			<div className="flex items-center justify-end gap-2 w-full text-sm text-muted">
				Block: {state.data.currentBlock} | Epoch:{" "}
				<Link to="/epoch" className="hover:opacity-75 transition">
					{state.data.currentEpoch}
				</Link>{" "}
				| GroupId:{" "}
				<Link to="/epoch" className="hover:opacity-75 transition">
					{state.data.currentGroupId.slice(0, 10)}
				</Link>
			</div>
		</header>
	);
}
