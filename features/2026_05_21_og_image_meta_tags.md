# Feature Proposal: Add OG/Twitter Image Meta Tags
Component: `explorer`

---

## Overview

Sharing the Explorer URL on X (Twitter), Slack, Discord, LinkedIn, and iMessage produces a blank link preview card. The `explorer/index.html` already declares `og:title`, `og:description`, and `twitter:card` but is missing `og:image`, `og:url`, and `twitter:image`. Social crawlers skip the card entirely without these. This adds the missing tags and places the preview image in the correct public directory.

Single PR — no phase split required.

---

## Architecture Decision

The OG preview image must be served at a stable, absolute URL so that the `.eth.limo` IPFS gateway and alternate deploy hosts can resolve it without depending on the page's own origin. Therefore the image is placed in `explorer/public/` (Vite serves it at the path root, no content-hashing) and referenced via `%VITE_APP_URL%`, which Vite substitutes at build time from the `VITE_APP_URL` environment variable.

### Alternatives Considered

- **Use `logo512.png`** — already in `public/`, but it is a 512×512 square logo. `summary_large_image` cards render square images as small thumbnails rather than full-width heroes. The dedicated 2400×1260 image provides better presentation.
- **Use a relative URL** — would break on IPFS gateways where the document origin differs from the asset origin.

---

## User Flow

No user-facing UI change. The benefit is visible only when a link is shared externally.

---

## Tech Specs

**New file:** `explorer/public/og-image.png` — 2400×1260 PNG (2× retina, 1200:630 aspect ratio; optimal for `summary_large_image` cards)

**Modified file:** `explorer/index.html` — new tags added:
- `og:url` — `%VITE_APP_URL%`
- `og:image` — `%VITE_APP_URL%/og-image.png`
- `og:image:width` / `og:image:height` — `2400` / `1260`
- `og:image:alt` — `"Safenet Explorer"`
- `twitter:image` — `%VITE_APP_URL%/og-image.png`
- `twitter:image:alt` — `"Safenet Explorer"`

**Modified file:** `explorer/.env.sample` — documents `VITE_APP_URL` (canonical public URL, no trailing slash)

---

## Implementation Phases

**Phase 1 (single PR):**
- Add `explorer/public/og-image.png` (2400×1260 preview image)
- Add missing meta tags to `explorer/index.html` using `%VITE_APP_URL%`
- Document `VITE_APP_URL` in `explorer/.env.sample`

---

## Open Questions / Assumptions

- Each deployment must set `VITE_APP_URL` to its canonical public URL at build time. If unset, `%VITE_APP_URL%` will appear literally in the rendered HTML and the meta tags will be non-functional.
