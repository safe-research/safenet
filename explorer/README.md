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

## Deployment

### IPFS

The explorer can be deployed to IPFS via Pinata:

```sh
npm run deploy:ipfs -w explorer
```

This builds the app and uploads the `dist/` directory to Pinata. The upload is always named with a snapshot timestamp (e.g. `safenet-explorer-2026-03-18T14:30:00.000Z`). Set `PINATA_JWT` (required) and `PINATA_GATEWAY` (optional) in your environment before running. To skip the build step and upload an existing `dist/`, pass `--skip-build`:

```sh
npm run deploy:ipfs -w explorer -- --skip-build

```

## Environment Variables

All variables are optional. Copy `.env.sample` to `.env` and fill in the values you need:

```sh
cp explorer/.env.sample explorer/.env
```

Set them in a `.env` file inside the `explorer/` directory, or as build-time environment variables.

| Variable | Default | Description |
|---|---|---|
| `VITE_BASE_PATH` | `/` | Base path when the app is served from a sub-path (e.g. `/explorer/`). |
| `VITE_DOCS_URL` | `https://docs.safefoundation.org/safenet` | URL for the "Docs" link in the footer. |
| `VITE_TERMS_URL` | `#tos` | URL for the "Terms" link in the footer. |
| `VITE_PRIVACY_URL` | `#privacy` | URL for the "Privacy" link in the footer. |
| `VITE_IMPRINT_URL` | `#imprint` | URL for the "Imprint" link in the footer. |
| `VITE_PLAUSIBLE_DOMAIN` | — | Plausible site domain (e.g. `explorer.safenet.io`). When set, Plausible tracking is initialized. When unset, no analytics run. |
| `VITE_PLAUSIBLE_ENDPOINT` | `https://plausible.io/api/event` | Full URL of the Plausible API endpoint. Override for self-hosted Plausible instances. |

## Analytics Integration

The explorer ships with a Plausible Analytics integration in `src/components/Analytics.tsx` using the [`@plausible-analytics/tracker`](https://www.npmjs.com/package/@plausible-analytics/tracker) npm package. The tracker is bundled with the application — no external script is fetched at runtime. The component is rendered first in the root layout, so it is present on every page.

To enable Plausible, set `VITE_PLAUSIBLE_DOMAIN` to your site's domain. If the variable is not set, no analytics are initialized.

```sh
# explorer/.env
VITE_PLAUSIBLE_DOMAIN=explorer.safenet.io
```

For self-hosted Plausible, additionally set `VITE_PLAUSIBLE_ENDPOINT`:

```sh
VITE_PLAUSIBLE_DOMAIN=explorer.safenet.io
VITE_PLAUSIBLE_ENDPOINT=https://plausible.example.com/api/event
```

SPA page-view tracking works automatically — the tracker hooks into the History API (`pushState`/`popstate`) via `autoCapturePageviews` (enabled by default), so all client-side navigations are captured without manual instrumentation.

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
