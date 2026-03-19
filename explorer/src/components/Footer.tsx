interface FooterLinkProps {
	href: string;
	children: React.ReactNode;
}

function FooterLink({ href, children }: FooterLinkProps) {
	if (!href) {
		return <span className="text-muted">{children}</span>;
	}
	return (
		<a href={href} target="_blank" rel="noopener noreferrer" className="text-muted hover:text-title transition-colors">
			{children}
		</a>
	);
}

export default function Footer() {
	return (
		<footer className="w-full border-t border-surface-outline bg-surface-1 mt-8">
			<div className="max-w-4xl mx-auto px-4 py-6 flex flex-col items-center gap-3 text-sm">
				<p className="text-muted">© Safenet / Safe Ecosystem Foundation</p>
				<nav className="flex flex-wrap justify-center gap-x-4 gap-y-2" aria-label="Footer navigation">
					<FooterLink href={__TERMS_URL__}>Terms</FooterLink>
					<FooterLink href={__PRIVACY_URL__}>Privacy</FooterLink>
					<FooterLink href={__IMPRINT_URL__}>Imprint</FooterLink>
					<FooterLink href={__DOCS_URL__}>Docs ↗</FooterLink>
				</nav>
			</div>
		</footer>
	);
}
