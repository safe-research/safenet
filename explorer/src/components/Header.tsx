import { Bars3Icon, XMarkIcon } from "@heroicons/react/24/solid";
import { Link } from "@tanstack/react-router";
import { useState } from "react";
import { SafenetBetaLogo } from "@/components/common/SafenetBetaLogo";
import { useConsensusState } from "@/hooks/useConsensusState";

export default function Header() {
	const state = useConsensusState();
	const [isOpen, setIsOpen] = useState(false);
	const close = () => setIsOpen(false);
	return (
		<header className="sticky top-0 z-50 w-full flex flex-col gap-1 px-4 py-2 bg-surface-1 border-b border-surface-outline">
			{/* Main bar — 3-col on desktop, logo + hamburger on mobile */}
			<div className="flex items-start w-full">
				{/* Left col: logo */}
				<div className="flex-1">
					<Link to="/" className="hover:opacity-75 transition inline-block" search={{}}>
						<SafenetBetaLogo />
					</Link>
				</div>

				{/* Centre col: nav links (desktop only) */}
				<nav className="hidden md:flex flex-1 items-start justify-center gap-6">
					<Link to="/" className="text-base text-muted hover:text-title transition-colors" search={{}}>
						Explore
					</Link>
					<Link to="/settings" className="text-base text-muted hover:text-title transition-colors">
						Settings
					</Link>
				</nav>

				{/* Right col: Docs (desktop only) + hamburger (mobile only) */}
				<div className="flex flex-1 items-start justify-end gap-3 pt-1">
					<a
						href={__DOCS_URL__}
						target="_blank"
						rel="noopener noreferrer"
						className="hidden md:inline text-sm text-muted hover:text-title transition-colors whitespace-nowrap"
					>
						Docs ↗
					</a>
					<button
						type="button"
						className="md:hidden p-1 -mt-1 text-muted hover:text-title transition-colors"
						aria-label={isOpen ? "Close menu" : "Open menu"}
						onClick={() => setIsOpen((o) => !o)}
					>
						{isOpen ? <XMarkIcon className="size-6" /> : <Bars3Icon className="size-6" />}
					</button>
				</div>
			</div>

			{/* Mobile dropdown — nav links, Docs and stats */}
			{isOpen && (
				<div className="md:hidden flex flex-col py-2 border-t border-surface-outline">
					<nav className="flex flex-col gap-3 pb-3">
						<Link
							to="/"
							className="text-base text-muted hover:text-title transition-colors"
							search={{}}
							onClick={close}
						>
							Explore
						</Link>
						<Link to="/settings" className="text-base text-muted hover:text-title transition-colors" onClick={close}>
							Settings
						</Link>
						<a
							href={__DOCS_URL__}
							target="_blank"
							rel="noopener noreferrer"
							className="text-base text-muted hover:text-title transition-colors"
							onClick={close}
						>
							Docs ↗
						</a>
					</nav>
					<div className="flex flex-col gap-1 pt-3 border-t border-surface-outline text-sm text-muted">
						<span>Block: {state.data.currentBlock}</span>
						<span>
							Epoch:{" "}
							<Link to="/epoch" className="hover:opacity-75 transition" onClick={close}>
								{state.data.currentEpoch}
							</Link>
						</span>
						<span>
							GroupId:{" "}
							<Link to="/epoch" className="hover:opacity-75 transition" onClick={close}>
								{state.data.currentGroupId.slice(0, 10)}
							</Link>
						</span>
					</div>
				</div>
			)}

			{/* Status row — desktop only */}
			<div className="hidden md:flex items-center justify-end gap-2 w-full text-sm text-muted">
				<span>Block: {state.data.currentBlock}</span>
				<span>|</span>
				<span>
					Epoch:{" "}
					<Link to="/epoch" className="hover:opacity-75 transition">
						{state.data.currentEpoch}
					</Link>
				</span>
				<span>|</span>
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
