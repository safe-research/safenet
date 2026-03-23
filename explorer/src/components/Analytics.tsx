import type { ReactNode } from "react";

/**
 * Analytics integration point.
 *
 * This component is intentionally a pass-through. Forks that want to add
 * analytics (e.g. Google Analytics, Plausible, Mixpanel) should replace this
 * file with their own implementation.
 *
 * The component wraps the entire application via the root layout, so it is
 * present on every page of the explorer. Wrapping children allows forks to
 * provide analytics context (e.g. a usePlausible hook) to the rest of the app.
 */
export default function Analytics({ children }: { children: ReactNode }) {
	return <>{children}</>;
}
