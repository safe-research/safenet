export interface LegalPage {
	label: string;
	/** In-app route rendering the page, linked when its content is bundled. */
	to: "/terms" | "/privacy" | "/imprint";
	/** Whether VITE_<KEY>_URL was a file path. Compared here rather than passing the HTML on, so the
	 * minifier folds it to a constant and the content stays in the route's lazy chunk. */
	bundled: boolean;
	/** External fallback URL, used when no content is bundled. */
	url: string;
}

export const LEGAL_PAGES: LegalPage[] = [
	{ label: "Terms", to: "/terms", bundled: __TERMS_HTML__ !== "", url: __TERMS_URL__ },
	{ label: "Privacy", to: "/privacy", bundled: __PRIVACY_HTML__ !== "", url: __PRIVACY_URL__ },
	{ label: "Imprint", to: "/imprint", bundled: __IMPRINT_HTML__ !== "", url: __IMPRINT_URL__ },
];
