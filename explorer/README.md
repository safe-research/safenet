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
| `VITE_PLAUSIBLE_DOMAIN` | — | Plausible site domain (e.g. `explorer.safenet.io`). When set, the Plausible script is injected. When unset, no analytics script is loaded. |
| `VITE_PLAUSIBLE_SCRIPT_URL` | `https://plausible.io/js/script.js` | URL of the Plausible script. Override for self-hosted Plausible instances. |

## Analytics Integration

The explorer ships with a Plausible Analytics integration in `src/components/Analytics.tsx`. The component is rendered first in the root layout, so it is present on every page.

To enable Plausible, set `VITE_PLAUSIBLE_DOMAIN` to your site's domain. If the variable is not set, no analytics script is injected.

```sh
# explorer/.env
VITE_PLAUSIBLE_DOMAIN=explorer.safenet.io
```

For self-hosted Plausible, additionally set `VITE_PLAUSIBLE_SCRIPT_URL`:

```sh
VITE_PLAUSIBLE_DOMAIN=explorer.safenet.io
VITE_PLAUSIBLE_SCRIPT_URL=https://plausible.example.com/js/script.js
```

React 19 hoists `<script>` tags rendered by components into `<head>` automatically, so no manual DOM manipulation is needed. React deduplicates the tag and moves it to `<head>`, matching the behaviour of Next.js `<Script>` components.

### Using a different analytics provider

To replace Plausible with another provider, overwrite `src/components/Analytics.tsx` with your own implementation. The component re-renders on every route change via the root layout, so implementations that track SPA navigation can call their page-view method inside a `useEffect`:

```tsx
// src/components/Analytics.tsx
import { useEffect } from "react";
import { useLocation } from "@tanstack/react-router";

export default function Analytics() {
  const location = useLocation();

  useEffect(() => {
    // Called on every route change because this component lives in the root layout.
    myAnalytics.page({ path: location.pathname });
  }, [location.pathname]);

  return null;
}
```

Because `<Analytics />` is the first element in the root layout, it initializes before `<Header />` or any page content renders.
