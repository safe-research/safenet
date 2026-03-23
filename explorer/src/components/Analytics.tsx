/**
 * Analytics integration point.
 *
 * By default this component loads Plausible Analytics when VITE_PLAUSIBLE_DOMAIN
 * is set. If the variable is not set, no analytics script is injected.
 *
 * To use a different analytics provider, replace this file with your own
 * implementation. The component is rendered once in the root layout, before any
 * page content, so it is present on every page of the explorer. Implementations
 * that track SPA navigation can call their page-view method inside a `useEffect`
 * — the component re-renders on every route change via the root layout.
 *
 * React 19 hoists `<script>` tags rendered by components into `<head>`
 * automatically, so no manual DOM manipulation is needed.
 */

const domain = import.meta.env.VITE_PLAUSIBLE_DOMAIN as string | undefined;
const scriptUrl = (import.meta.env.VITE_PLAUSIBLE_SCRIPT_URL as string | undefined) ?? "https://plausible.io/js/script.js";

export default function Analytics() {
	if (!domain) return null;

	return <script defer src={scriptUrl} data-domain={domain} />;
}
