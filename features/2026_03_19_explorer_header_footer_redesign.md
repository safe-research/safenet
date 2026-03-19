# Feature Proposal: Explorer Header & Footer Redesign
Component: `explorer`

---

## Overview

Redesign the explorer's header and footer to improve navigation, compliance, and UX. Key changes:

- **Remove** the beta warning banner from the header
- **Add** named navigation links (Explore, Settings) and an external Docs link to the header
- **Responsive header**: on small screens, status info (block, epoch, group) wraps to a second row
- **Replace** the old "Built by Safe Research" footer with a compliance footer containing copyright and legal/docs links (Terms, Privacy, Imprint, Docs)
- All external link URLs are configurable via build-time environment variables

This is a single focused PR covering the full header/footer redesign.

User stories: **E500**, **E501**

---

## Architecture Decision

### Header

The existing `Header.tsx` will be refactored to:

1. Replace the `SafeResearchBanner` (beta warning) with nothing — the banner component is removed.
2. Add a horizontal nav row with text links: **[Explore]** (→ `/`) and **[Settings]** (→ `/settings`), plus an external **[Docs ↗]** link.
3. Keep the status info section (block number, epoch, group id) and the gear `Cog6ToothIcon` settings link as-is (possibly adjust font style to `text-muted`/`text-sm`).
4. On small screens (`< md` breakpoint), the nav row and status row stack vertically so the status info moves below the logo/nav.

Layout (large screen):
```
+----------------------------------------------------------------------------------+
| <Safenet Logo>   [Explore] [Settings]                           [Docs ↗]        |
|                                        Block: X | Epoch: Y | GroupId: Z  [⚙]   |
+----------------------------------------------------------------------------------+
```

Layout (small screen):
```
+----------------------------------------------+
| <Safenet Logo>               [Docs ↗]        |
| [Explore] [Settings]                   [⚙]  |
| Block: X | Epoch: Y | GroupId: Z             |
+----------------------------------------------+
```

### Footer

A new `Footer.tsx` component replaces `SafeResearchFooter` from `SafeResearch.tsx`. It renders:

```
+----------------------------------------------------------------------------------+
| © Safenet / Safe Ecosystem Foundation                                            |
| Terms | Privacy | Imprint | Docs ↗                                              |
+----------------------------------------------------------------------------------+
```

Link URLs are read from Vite-defined constants (build variables). Links with empty URLs are still displayed (rendered as `<span>` or `<a href="">` for accessibility) to ensure Terms/Privacy/Imprint always appear even before real URLs are configured.

### Build Variables

Defined in `vite.config.js` via `define`, using `loadEnv` to pick up from `.env` files:

| Env variable        | Constant          | Default                                  |
|---------------------|-------------------|------------------------------------------|
| `VITE_DOCS_URL`     | `__DOCS_URL__`    | `https://docs.safefoundation.org/safenet` |
| `VITE_TERMS_URL`    | `__TERMS_URL__`   | `""`                                      |
| `VITE_PRIVACY_URL`  | `__PRIVACY_URL__` | `""`                                      |
| `VITE_IMPRINT_URL`  | `__IMPRINT_URL__` | `""`                                      |

### SafeResearch.tsx

Both `SafeResearchBanner` and `SafeResearchFooter` are removed (file can be deleted if no other usages remain).

### Alternatives Considered

- **Keep `SafeResearch.tsx` and extend it** — rejected to avoid coupling unrelated compliance links with the old Safe Research branding.
- **Inline link URLs as constants** — rejected in favour of build variables to allow deployment-specific overrides without code changes.
- **Route-level footers** — rejected; a single root-level footer is simpler and consistent with the existing layout pattern.

---

## User Flow

### E500 — Docs Navigation

1. User visits the explorer on any page.
2. User sees **[Docs ↗]** in the header and also in the footer.
3. Clicking either opens `https://docs.safefoundation.org/safenet` in a new tab.

### E501 — Compliance Links

1. User scrolls to the bottom of any page.
2. Footer displays: `© Safenet / Safe Ecosystem Foundation` and links `Terms | Privacy | Imprint | Docs ↗`.
3. All links open in a new tab.

### Page Layout

**Header (large screen):**
```
+----------------------------------------------------------------------------------+
| <Safenet Logo>   [Explore] [Settings]                           [Docs ↗]        |
|                                        Block: X | Epoch: Y | GroupId: Z  [⚙]   |
+----------------------------------------------------------------------------------+
```

**Header (small screen, stacked):**
```
+-----------------------------------------------+
| <Safenet Logo>                    [Docs ↗]    |
| [Explore] [Settings]                    [⚙]  |
| Block: X | Epoch: Y | GroupId: Z              |
+-----------------------------------------------+
```

**Footer:**
```
+----------------------------------------------------------------------------------+
| © Safenet / Safe Ecosystem Foundation                                            |
| Terms | Privacy | Imprint | Docs ↗                                              |
+----------------------------------------------------------------------------------+
```

---

## Tech Specs

### New / Modified Files

| File | Action |
|------|--------|
| `explorer/src/components/Header.tsx` | Modify — remove banner, add nav links + Docs, responsive layout |
| `explorer/src/components/Footer.tsx` | **Create** — new compliance footer |
| `explorer/src/components/SafeResearch.tsx` | Delete — no longer used |
| `explorer/src/routes/__root.tsx` | Modify — replace `SafeResearchFooter` with `<Footer />` |
| `explorer/vite.config.js` | Modify — add `define` entries for link URL constants |

### Environment Variables

Loaded via `loadEnv` in `vite.config.js` (same pattern as `VITE_BASE_PATH`):

```js
define: {
  __DOCS_URL__:    JSON.stringify(env.VITE_DOCS_URL    || "https://docs.safefoundation.org/safenet"),
  __TERMS_URL__:   JSON.stringify(env.VITE_TERMS_URL   || ""),
  __PRIVACY_URL__: JSON.stringify(env.VITE_PRIVACY_URL || ""),
  __IMPRINT_URL__: JSON.stringify(env.VITE_IMPRINT_URL || ""),
}
```

TypeScript declarations for these globals should be added in `explorer/src/vite-env.d.ts` (or a new `global.d.ts`).

### Components

**`Footer.tsx`**
- Props: none (reads from build constants)
- Renders copyright line and link row
- External links: `target="_blank" rel="noopener noreferrer"`
- Links with empty URL: rendered as plain `<span>` (no `href`) to remain visible but non-navigable

**`Header.tsx` changes**
- Remove `<SafeResearchBanner />` import and usage
- Add `[Explore]` link (`<Link to="/">`) and `[Settings]` link (`<Link to="/settings">`) as text nav items
- Add `[Docs ↗]` external anchor (`target="_blank"`)
- Responsive: wrap the status row below nav on `< md` screens using Tailwind `flex-col md:flex-row` / `hidden md:flex` approach

### Existing Utilities to Reuse

- `Link` from `@tanstack/react-router` — used for internal nav links (already in Header.tsx)
- `cn` from `@/lib/utils` — for conditional class merging
- `useConsensusState` hook at `explorer/src/hooks/useConsensusState.tsx` — keep as-is for status data
- `Cog6ToothIcon` from `@heroicons/react/24/solid` — keep settings icon
- Tailwind color tokens (`text-muted`, `text-title`, `bg-surface-1`, `border-surface-outline`) — use for new links and footer styling

### Tests

- Unit tests for `Footer.tsx`: renders copyright text; renders Docs link with correct href and `target="_blank"`; renders Terms/Privacy/Imprint as `<span>` when URL is empty; renders as `<a>` when URL is non-empty.
- Unit tests for `Header.tsx`: renders Explore and Settings nav links; renders Docs external link; does not render beta warning banner.

---

## Implementation Phases

### Phase 1 — Single PR: Header & Footer Redesign

All changes are small and tightly related; a single PR is appropriate.

**Files touched:**
- `explorer/src/components/Header.tsx`
- `explorer/src/components/Footer.tsx` (new)
- `explorer/src/components/SafeResearch.tsx` (deleted)
- `explorer/src/routes/__root.tsx`
- `explorer/vite.config.js`
- `explorer/src/vite-env.d.ts` (or new `global.d.ts` for constant types)
- Unit test files for `Header` and `Footer`

**Checklist:**
- [ ] Remove `SafeResearchBanner` from header
- [ ] Add Explore, Settings text nav links
- [ ] Add Docs ↗ external link to header
- [ ] Responsive two-row header on small screens
- [ ] Create `Footer.tsx` with copyright + legal links
- [ ] Wire build variables in `vite.config.js`
- [ ] Add TypeScript globals declarations
- [ ] Delete `SafeResearch.tsx`, update `__root.tsx`
- [ ] Write unit tests
- [ ] `npm run check` passes
- [ ] `npm test` passes

---

## Open Questions / Assumptions

- **Propose link**: omitted from header for now (no user story covers it; can be added in a future PR).
- **Links with empty URL**: displayed as `<span>` elements so Terms/Privacy/Imprint always appear in the footer even when URLs are not yet configured.
- **Status info font style**: minor Tailwind class adjustments (e.g. `text-sm text-muted`) are acceptable per the spec note "at most adjust font styles".
- **`SafenetBetaLogo`**: the existing logo SVG component is kept — only the "Beta" label in the aria-label and the component name may need updating in a future PR; out of scope here.
