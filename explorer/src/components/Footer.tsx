import { Link } from "@tanstack/react-router";
import { LEGAL_PAGES } from "@/lib/legal";

const LINK_CLASS = "text-muted hover:text-title transition-colors";

interface FooterLinkProps {
	href: string;
	children: React.ReactNode;
}

function FooterLink({ href, children }: FooterLinkProps) {
	return (
		<a href={href} target="_blank" rel="noopener noreferrer" className={LINK_CLASS}>
			{children}
		</a>
	);
}

export default function Footer() {
	return (
		<footer className="w-full border-t border-surface-outline bg-surface-1 mt-8">
			<div className="max-w-4xl mx-auto px-4 py-6 flex flex-col items-center gap-3 text-sm">
				<nav className="flex flex-wrap justify-center gap-x-4 gap-y-2" aria-label="Footer navigation">
					{LEGAL_PAGES.map(({ label, to, bundled, url }) =>
						bundled ? (
							<Link key={label} to={to} className={LINK_CLASS}>
								{label}
							</Link>
						) : (
							<FooterLink key={label} href={url}>
								{label}
							</FooterLink>
						),
					)}
					<FooterLink href={__DOCS_URL__}>Docs ↗</FooterLink>
				</nav>
			</div>
		</footer>
	);
}
