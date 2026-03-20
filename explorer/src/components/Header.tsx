import { Bars3Icon, XMarkIcon } from "@heroicons/react/24/solid";
import { Link } from "@tanstack/react-router";
import { useState } from "react";
import { Button } from "@/components/common/Button";
import { SafenetBetaLogo } from "@/components/common/SafenetBetaLogo";
import { useConsensusState } from "@/hooks/useConsensusState";

type NavLinkProps = {
	children: React.ReactNode;
	className?: string;
	onClick?: () => void;
} & ({ to: string; search?: Record<string, unknown>; href?: never } | { href: string; to?: never; search?: never });

function NavLink({ children, className, onClick, ...rest }: NavLinkProps) {
	const cls = `text-muted hover:text-title transition-colors${className ? ` ${className}` : ""}`;
	if ("href" in rest && rest.href) {
		return (
			<a href={rest.href} target="_blank" rel="noopener noreferrer" className={cls} onClick={onClick}>
				{children}
			</a>
		);
	}
	const { to, search } = rest as { to: string; search?: Record<string, unknown> };
	return (
		<Link to={to} search={search ?? {}} className={cls} onClick={onClick}>
			{children}
		</Link>
	);
}

function StatLink({ to, children, onClick }: { to: string; children: React.ReactNode; onClick?: () => void }) {
	return (
		<Link to={to} className="hover:opacity-75 transition" onClick={onClick}>
			{children}
		</Link>
	);
}

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
					<NavLink to="/" search={{}} className="text-base">
						Explore
					</NavLink>
					<NavLink to="/settings" className="text-base">
						Settings
					</NavLink>
				</nav>

				{/* Right col: Docs (desktop only) + hamburger (mobile only) */}
				<div className="flex flex-1 items-start justify-end gap-3 pt-1">
					<NavLink href={__DOCS_URL__} className="hidden md:inline text-sm whitespace-nowrap">
						Docs ↗
					</NavLink>
					<Button
						variant="ghost"
						className="md:hidden p-1 -mt-1 text-muted"
						aria-label={isOpen ? "Close menu" : "Open menu"}
						onClick={() => setIsOpen((o) => !o)}
					>
						{isOpen ? <XMarkIcon className="size-6" /> : <Bars3Icon className="size-6" />}
					</Button>
				</div>
			</div>

			{/* Mobile dropdown — nav links, Docs and stats */}
			{isOpen && (
				<div className="md:hidden flex flex-col py-2 border-t border-surface-outline">
					<nav className="flex flex-col gap-3 pb-3">
						<NavLink to="/" search={{}} className="text-base" onClick={close}>
							Explore
						</NavLink>
						<NavLink to="/settings" className="text-base" onClick={close}>
							Settings
						</NavLink>
						<NavLink href={__DOCS_URL__} className="text-base" onClick={close}>
							Docs ↗
						</NavLink>
					</nav>
					<div className="flex flex-col gap-1 pt-3 border-t border-surface-outline text-sm text-muted">
						<span>Block: {state.data.currentBlock}</span>
						<span>
							Epoch:{" "}
							<StatLink to="/epoch" onClick={close}>
								{state.data.currentEpoch}
							</StatLink>
						</span>
						<span>
							GroupId:{" "}
							<StatLink to="/epoch" onClick={close}>
								{state.data.currentGroupId.slice(0, 10)}
							</StatLink>
						</span>
					</div>
				</div>
			)}

			{/* Status row — desktop only */}
			<div className="hidden md:flex items-center justify-end gap-2 w-full text-sm text-muted">
				<span>Block: {state.data.currentBlock}</span>
				<span>|</span>
				<span>
					Epoch: <StatLink to="/epoch">{state.data.currentEpoch}</StatLink>
				</span>
				<span>|</span>
				<span>
					GroupId: <StatLink to="/epoch">{state.data.currentGroupId.slice(0, 10)}</StatLink>
				</span>
			</div>
		</header>
	);
}
