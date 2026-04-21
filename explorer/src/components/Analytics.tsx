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

// Called at module load time — runs exactly once regardless of React's
// component lifecycle, so no guard or useEffect is needed.
const domain = import.meta.env.VITE_PLAUSIBLE_DOMAIN as string | undefined;
const endpoint = import.meta.env.VITE_PLAUSIBLE_ENDPOINT as string | undefined;

if (domain) {
	init({ domain, hashBasedRouting: true, ...(endpoint ? { endpoint } : {}) });
}

export default function Analytics() {
	return null;
}
