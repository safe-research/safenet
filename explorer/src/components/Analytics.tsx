/**
 * Analytics integration point.
 *
 * By default this component initializes Plausible Analytics when
 * VITE_PLAUSIBLE_DOMAIN is set. If the variable is not set, no tracking is
 * initialized. The Plausible tracker package is bundled with the application
 * — no external script is fetched at runtime.
 *
 * `autoCapturePageviews` (enabled by default) hooks into the History API so
 * all SPA navigations are tracked automatically without manual instrumentation.
 *
 * To use a different analytics provider, replace this file with your own
 * implementation. The component is rendered once in the root layout, before
 * any page content, so it is present on every page of the explorer.
 */
import { init } from "@plausible-analytics/tracker";
import { useEffect } from "react";

// Module-level guard so init() is only called once. Without this, React
// StrictMode's intentional double-mount in development causes a second
// useEffect call which makes the Plausible SDK throw.
let initialized = false;

export default function Analytics() {
	useEffect(() => {
		const domain = import.meta.env.VITE_PLAUSIBLE_DOMAIN as string | undefined;
		const endpoint = import.meta.env.VITE_PLAUSIBLE_ENDPOINT as string | undefined;
		if (!domain || initialized) return;
		initialized = true;
		init({ domain, ...(endpoint ? { endpoint } : {}) });
	}, []);

	return null;
}
