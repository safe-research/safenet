import { Bars3Icon, XMarkIcon } from "@heroicons/react/24/solid";
import { Link } from "@tanstack/react-router";
import { useState } from "react";
import { SafenetBetaLogo } from "@/components/common/SafenetBetaLogo";
import { useConsensusState } from "@/hooks/useConsensusState";

export default function Header() {
	const state = useConsensusState();
	const [isOpen, setIsOpen] = useState(false);
	return (
		<header className="sticky top-0 z-50 w-full flex flex-col gap-1 px-4 py-2 bg-surface-1 border-b border-surface-outline">
			{/* Main bar — 3-col on desktop, logo + controls on mobile */}
			<div className="flex items-center w-full">
				{/* Left col: logo */}
				<div className="flex-1">
					<Link to="/" className="hover:opacity-75 transition inline-block" search={{}}>
						<SafenetBetaLogo />
					</Link>
				</div>

				{/* Centre col: nav links (desktop only) */}
				<nav className="hidden md:flex flex-1 items-center justify-center gap-6">
					<Link to="/" className="text-base text-muted hover:text-title transition-colors" search={{}}>
						Explore
					</Link>
					<Link to="/settings" className="text-base text-muted hover:text-title transition-colors">
						Settings
					</Link>
				</nav>

				{/* Right col: Docs + hamburger */}
				<div className="flex flex-1 items-center justify-end gap-3">
					<a
						href={__DOCS_URL__}
						target="_blank"
						rel="noopener noreferrer"
						className="text-sm text-muted hover:text-title transition-colors whitespace-nowrap"
					>
						Docs ↗
					</a>
					<button
						type="button"
						className="md:hidden p-1 text-muted hover:text-title transition-colors"
						aria-label={isOpen ? "Close menu" : "Open menu"}
						onClick={() => setIsOpen((o) => !o)}
					>
						{isOpen ? <XMarkIcon className="size-6" /> : <Bars3Icon className="size-6" />}
					</button>
				</div>
			</div>

			{/* Mobile nav dropdown */}
			{isOpen && (
				<nav className="md:hidden flex flex-col gap-3 py-2 border-t border-surface-outline">
					<Link
						to="/"
						className="text-base text-muted hover:text-title transition-colors"
						search={{}}
						onClick={() => setIsOpen(false)}
					>
						Explore
					</Link>
					<Link
						to="/settings"
						className="text-base text-muted hover:text-title transition-colors"
						onClick={() => setIsOpen(false)}
					>
						Settings
					</Link>
				</nav>
			)}

			{/* Status row — stacked on mobile, inline on desktop */}
			<div className="flex flex-col md:flex-row md:items-center md:justify-end items-end gap-1 md:gap-2 w-full text-sm text-muted">
				<span>Block: {state.data.currentBlock}</span>
				<span className="hidden md:inline">|</span>
				<span>
					Epoch:{" "}
					<Link to="/epoch" className="hover:opacity-75 transition">
						{state.data.currentEpoch}
					</Link>
				</span>
				<span className="hidden md:inline">|</span>
				<span>
					GroupId:{" "}
					<Link to="/epoch" className="hover:opacity-75 transition">
						{state.data.currentGroupId.slice(0, 10)}
					</Link>
				</span>
			</div>
		</header>
	);
}
