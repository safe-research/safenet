# Safenet Explorer

The Safenet Explorer is a React 19 single-page application for inspecting the state of the Safenet network: transaction proposals, epoch information, validator status, and consensus state.

## Stack

- **React 19** + **TypeScript 5**
- **TanStack Router** — file-based routing with type-safe search params
- **TanStack Query** — data fetching and caching
- **Tailwind CSS 4** — styling with a custom dark/light theme
- **Vite 7** — build tool

## Development

### Prerequisites

Install dependencies from the repository root:

```sh
npm install
```

### Run the dev server

```sh
npm run dev -w explorer        # starts on http://localhost:3000
```

### Build

```sh
npm run build -w explorer
```

### Lint and type-check

```sh
npm run check -w explorer      # Biome + TypeScript
npm run fix -w explorer        # auto-fix formatting issues
```

### Tests

```sh
npm test -w explorer
```

## Environment Variables

All variables are optional. Set them in a `.env` file inside the `explorer/` directory, or as build-time environment variables.

| Variable | Default | Description |
|---|---|---|
| `VITE_BASE_PATH` | `/` | Base path when the app is served from a sub-path (e.g. `/explorer/`). |
| `VITE_DOCS_URL` | `https://docs.safefoundation.org/safenet` | URL for the "Docs" link in the footer. |
| `VITE_TERMS_URL` | `#tos` | URL for the "Terms" link in the footer. |
| `VITE_PRIVACY_URL` | `#privacy` | URL for the "Privacy" link in the footer. |
| `VITE_IMPRINT_URL` | `#imprint` | URL for the "Imprint" link in the footer. |

## Analytics Integration

The explorer ships with a no-op `Analytics` component (`src/components/Analytics.tsx`) that is rendered first in the root layout, making it present on every page. By default it does nothing and returns `null`.

**Forks that want to add analytics should replace this file** with their own implementation. The component is intentionally kept minimal so no assumptions are made about which analytics provider is used.

### Example: Plausible Analytics

The example below mirrors the [`PlausibleProvider`](https://github.com/4lejandrito/next-plausible) pattern for Vite / React 19. React 19 hoists `<script>` tags rendered inside components into `<head>` automatically, so no Next.js `<Script>` component is needed. Wrapping `children` makes the `usePlausible` hook available throughout the app, matching the ergonomics of `next-plausible`.

```tsx
// src/components/Analytics.tsx
import { createContext, useContext, type ReactNode } from "react";

type PlausibleFn = (
  event: string,
  options?: { props?: Record<string, string | number | boolean> },
) => void;

const PlausibleContext = createContext<PlausibleFn>(() => {});

/** Track a custom Plausible event from any component. */
export function usePlausible() {
  return useContext(PlausibleContext);
}

export default function Analytics({ children }: { children: ReactNode }) {
  const plausible: PlausibleFn = (event, options) => {
    (window as Window & { plausible?: PlausibleFn }).plausible?.(event, options);
  };

  return (
    <PlausibleContext.Provider value={plausible}>
      {/* React 19 hoists this <script> into <head> automatically. */}
      <script defer src="https://plausible.io/js/script.js" data-domain="yourdomain.com" />
      {children}
    </PlausibleContext.Provider>
  );
}
```

Because `Analytics` wraps the entire app via the root layout, `usePlausible` is available in any component:

```tsx
import { usePlausible } from "@/components/Analytics";

export function MyButton() {
  const plausible = usePlausible();
  return <button onClick={() => plausible("ButtonClicked")}>Click me</button>;
}
```
