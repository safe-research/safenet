/**
 * Analytics integration point.
 *
 * By default this component initializes Plausible Analytics when
 * VITE_PLAUSIBLE_DOMAIN is set. If the variable is not set, no tracking is
 * initialized. The Plausible tracker package is bundled with the application
 * — no external script is fetched at runtime.
 *
 * `autoCapturePageviews` is disabled because TanStack Router's hash history
 * uses history.pushState to update the URL (not location.hash assignment), so
 * neither hashchange events nor Plausible's pathname comparison reflect actual
 * in-app navigations. Pageviews are tracked manually via useRouterState instead.
 * `hashBasedRouting` is kept so that the tracker includes h=1 in every event
 * payload, which tells the Plausible server to preserve the hash when recording
 * the URL (without it the server strips the hash and all pages report as /).
 *
 * To use a different analytics provider, replace this file with your own
 * implementation. The component is rendered once in the root layout, before
 * any page content, so it is present on every page of the explorer.
 */
import { init, track } from "@plausible-analytics/tracker";
import { useRouterState } from "@tanstack/react-router";
import { useEffect } from "react";

// Called at module load time — runs exactly once regardless of React's
// component lifecycle, so no guard or useEffect is needed.
const domain = import.meta.env.VITE_PLAUSIBLE_DOMAIN as string | undefined;
const endpoint = import.meta.env.VITE_PLAUSIBLE_ENDPOINT as string | undefined;

if (domain) {
	init({ domain, hashBasedRouting: true, autoCapturePageviews: false, ...(endpoint ? { endpoint } : {}) });
}

export default function Analytics() {
	const href = useRouterState({ select: (s) => s.location.href });

	useEffect(() => {
		if (domain) track("pageview");
	}, [href]);

	return null;
}
