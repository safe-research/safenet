/**
 * Analytics integration point.
 *
 * This component is intentionally empty. Forks that want to add analytics
 * (e.g. Google Analytics, Plausible, Mixpanel) should replace this file
 * with their own implementation.
 *
 * The component is rendered once in the root layout, before any page content,
 * so it is present on every page of the explorer. Implementations that track
 * SPA navigation can call their page-view method inside a `useEffect` — the
 * component re-renders on every route change via the root layout.
 */
export default function Analytics() {
	return null;
}
