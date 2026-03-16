# Feature Proposal: Explorer Design System
Component: `explorer`

---

## Overview

The explorer already has a solid foundation: Tailwind CSS 4 with CSS custom properties as design tokens, a `cn()` utility, and domain-organized components. However, the token layer is incomplete (missing `muted` text, no formal radius scale, no spacing aliases), a handful of components bypass the token system with raw Tailwind colours (`text-red-500`, `text-green-500`, `text-yellow-500`), and repeated Tailwind class strings are copy-pasted rather than encapsulated in primitives.

This specification formalises the existing approach — no new frameworks — into a proper design system:

1. **Phase 1 (PR 1)**: Audit and complete the token layer in `styles.css`
2. **Phase 2 (PR 2)**: Establish primitive component layer (`Button`, `Input`, `Label`, `Badge` variants)
3. **Phase 3 (PR 3)**: Standardise composite components — eliminate hardcoded colour bypasses and unify repeated class patterns
4. **Phase 4 (PR 4)**: Align colour palette, typography, and card visual style with the Safenet staking UI brand

Each phase produces a standalone, reviewable PR.

---

## Architecture Decision

The existing system uses **Tailwind CSS 4 `@theme` variables** as the single source of truth for design tokens, surfaced as utility classes (`text-title`, `bg-surface-1`, etc.). This is the correct pattern for this stack and will be preserved and extended.

No new libraries are introduced. The design system lives entirely in:

- `src/styles.css` — token definitions (`@theme` + `:root`/`.dark` CSS custom properties)
- `src/components/` — primitive and composite React components
- `src/lib/utils.ts` — `cn()` helper (already present)

### Token Layer

All semantic colour names, spacing aliases, typography scale, and radius values will be declared once in `src/styles.css` and consumed via Tailwind utilities everywhere else. No component will use a raw hex, oklch literal, or out-of-system Tailwind colour (e.g. `text-red-500`) after Phase 3.

**Colours to add/fix:**
- `--color-muted` / `text-muted` — secondary/helper text colour (currently missing; some components fall back to ad-hoc grays)
- Fix misleading comment on `--color-surface-1` in light mode (value is `oklch(1 0 0)` = white, not "black")

**Radius scale to formalise** — use use-case based naming, consistent with the colour token convention (e.g. `--color-button`, not `--color-sm`).

The current codebase uses `rounded-lg`, `rounded-md`, and `rounded-full` without clear use-case semantics:
- `rounded-lg` appears in `Box`, `Skeleton`, `SearchBar`, and `KeyGenStatusItem` — all cards/containers, but also in `CopyButton` which is not a card
- `rounded-md` appears in `FormItem` inputs and the `EpochRolloverItem` expand panel — two different use cases sharing the same class with no shared intent
- `rounded-full` appears in `Badge` — this one is clear, but unnamed

Formalising these as use-case tokens removes the ambiguity:

| Token | Value | Usage |
|---|---|---|
| `--radius-card` | `0.5rem` | `Box`, `Skeleton`, containers |
| `--radius-input` | `0.375rem` | Inputs, dropdowns |
| `--radius-badge` | `9999px` | Badges, pills |

Map these into `@theme` as `rounded-card`, `rounded-input`, `rounded-badge` so the intent is explicit at the call site.

**Typography scale** (currently only `--text-2xs` is custom):

| Token | Value | Usage |
|---|---|---|
| `--text-2xs` | `0.625rem` | Badge labels (already defined) |
| `--font-mono` | `source-code-pro, Menlo, Monaco, Consolas, 'Courier New', monospace` | Addresses, hashes, block numbers |

**Spacing** — no new scale needed; Tailwind's default 4px base unit is used consistently and works well.

### Component Layers

```
Primitive layer          Composite layer
───────────────          ────────────────
Button                   SearchBar
Input / Label            TransactionListRow
Badge (generalised)      SafeTxHeader, SafeTxSummary
Spinner                  EpochCard, EpochRolloverItem
Skeleton (exists)        KeyGenStatusItem
Box / Container (exist)  StatusBadge, NetworkBadge
```

Primitives accept a `className` prop threaded through `cn()`, allowing composites to override without forking.

### Alternatives Considered

| Alternative | Reason rejected |
|---|---|
| **shadcn/ui** | Introduces Radix UI and a large component set; violates "follow existing paradigms, no new frameworks" unless there is a clear gap. The existing component set covers the surface area needed. |
| **CSS Modules** | The codebase is 100% utility-first Tailwind; introducing CSS modules would fragment the styling model. |
| **Tailwind `@apply` layers** | `@apply` in Tailwind 4 is discouraged; component abstractions in React are the correct encapsulation unit. |
| **Separate design-tokens package** | Overkill for a single workspace; `styles.css` is the right home. |

---

## User Flow

No new user-facing flows. This is a developer-facing refactor. Visual output is identical or improved (more consistent spacing, radii, and colour contrast).

---

## Tech Specs

### New / Modified Files

| File | Change |
|---|---|
| `src/styles.css` | Add `--color-muted`, fix surface-1 comment, add use-case radius tokens, add `--font-mono`, add token-group comment headers. **Phase 4**: update token values to Safenet brand palette; add font imports, shadow tokens, set `--radius-card` to `0` |
| `src/components/common/Button.tsx` | New primitive — wraps `<button>` with variant props (`primary`, `ghost`, `icon`) |
| `src/components/common/Input.tsx` | New primitive — wraps `<input>` with consistent border/bg/text styling |
| `src/components/common/Label.tsx` | New primitive — `<label>` with consistent typography |
| `src/components/common/Badge.tsx` | Extend existing — add `variant` prop (`positive`, `pending`, `error`, `warning`, `neutral`) replacing ad-hoc inline styles |
| `src/components/common/Spinner.tsx` | Extract `LoadingSpinner` out of `Forms.tsx` into a standalone primitive |
| `src/components/Forms.tsx` | Refactor `FormItem`/`SubmitItem` to use new `Input`, `Label`, `Button`, `Spinner` primitives |
| `src/components/KeyGenStatusItem.tsx` | Replace `text-red-500`/`text-green-500`/`text-yellow-500` with token-based utilities |
| `src/components/NetworkBadge.tsx` | Replace hardcoded fallback `#4B5563` with `--color-muted` token |
| `src/components/TransactionListControls.tsx` | Replace local spinner with `Spinner` primitive |

### Token Inventory (after Phase 1)

**Colours** (all in `:root` / `.dark`, mapped in `@theme`):

```
--color-primary
--color-title
--color-title-hover
--color-sub-title
--color-muted              ← NEW
--color-surface-0
--color-surface-1
--color-surface-hover
--color-surface-outline
--color-button
--color-button-hover
--color-button-outline
--color-button-content
--color-positive
--color-positive-foreground
--color-pending
--color-pending-foreground
--color-error
--color-error-outline
--color-error-surface
--color-warning
--color-warning-outline
--color-warning-surface
```

**Radii** (new, use-case named, in `@theme`):

```
--radius-card  → rounded-card   (0.5rem  — containers, cards)
--radius-input → rounded-input  (0.375rem — inputs, dropdowns)
--radius-badge → rounded-badge  (9999px  — badges, pills)
```

**Typography** (additions to `@theme`):

```
--text-2xs     (already defined)
--font-mono    (new explicit alias)
```

### Button Variants

| Variant | Base classes |
|---|---|
| `primary` | `bg-button hover:bg-button-hover text-button-content rounded-input px-4 py-2` |
| `ghost` | `text-sub-title hover:text-title hover:underline` |
| `icon` | `inline-flex items-center text-xs px-1.5 py-0.5 border border-surface-outline rounded-input` |

### Badge Variants

| Variant | Tailwind classes |
|---|---|
| `positive` | `bg-positive text-positive-foreground` |
| `pending` | `bg-pending text-pending-foreground` |
| `error` | `bg-error-surface text-error border border-error-outline` |
| `warning` | `bg-warning-surface text-warning border border-warning-outline` |
| `neutral` | `bg-surface-0 text-muted border border-surface-outline` |

### Test Cases

All new primitives (`Button`, `Input`, `Label`, `Badge`, `Spinner`) require unit tests in `src/components/common/*.test.tsx` covering:

- Default render (snapshot or role assertions)
- Variant prop permutations render without crash
- `className` prop merges correctly via `cn()`
- Disabled state for `Button` and `Input`

Existing component tests (`StatusBadge.test.tsx`, `NetworkBadge.test.tsx`, etc.) must remain green after Phase 3 refactor.

---

## Implementation Phases

### Phase 1 — Token Layer (PR 1)

**Goal**: Make `src/styles.css` the complete, well-documented single source of truth.

Files touched:
- `src/styles.css`

Changes:
1. Add `--color-muted` (light: `oklch(44.6% 0.03 256.802)`, dark: `oklch(63% 0.02 256.802)`) and map in `@theme`. This is a new token for decorative/helper text; it is intentionally distinct from `--color-sub-title`, which is for structural secondary text (section labels, descriptions).
2. Fix the comment on `--color-surface-1` in light mode (says "black", value is correct white — comment-only fix, do not change the value).
3. Add `--radius-card` (0.5rem), `--radius-input` (0.375rem), `--radius-badge` (9999px) in `@theme` using use-case based naming (consistent with colour token convention, e.g. `--color-button` not `--color-sm`).
4. Add `--font-mono` in `@theme`
5. Add a comment block above each token group (Surfaces, Text, Button, Status, Radius, Typography) for discoverability

No component changes. No visual changes. `npm run check` must pass.

---

### Phase 2 — Primitive Components (PR 2)

**Goal**: Introduce `Button`, `Input`, `Label`, `Spinner` primitives and extend `Badge` with typed variants.

Files touched:
- `src/components/common/Button.tsx` (new)
- `src/components/common/Button.test.tsx` (new)
- `src/components/common/Input.tsx` (new)
- `src/components/common/Input.test.tsx` (new)
- `src/components/common/Label.tsx` (new)
- `src/components/common/Label.test.tsx` (new)
- `src/components/common/Spinner.tsx` (new — extracted from `Forms.tsx`)
- `src/components/common/Spinner.test.tsx` (new)
- `src/components/common/Badge.tsx` (extend existing)
- `src/components/common/Badge.test.tsx` (extend existing)

Constraints:
- Each primitive accepts a `className` prop merged via `cn()`
- No changes to consumers yet (Forms.tsx, StatusBadge, etc.) — that is Phase 3
- All tests green, `npm run check` passes

---

### Phase 3 — Composite Standardisation (PR 3)

**Goal**: Replace all token bypasses and inline class duplication in composite components with Phase 1 tokens and Phase 2 primitives.

Files touched:
- `src/components/Forms.tsx` — use `Input`, `Label`, `Button`, `Spinner`
- `src/components/common/StatusBadge.tsx` — use `Badge` variant prop
- `src/components/common/NetworkBadge.tsx` — replace hardcoded fallback `#4B5563` with `text-muted`
- `src/components/epoch/KeyGenStatusItem.tsx` — replace `text-red-500`/`text-green-500`/`text-yellow-500` with `text-error`/`text-positive`/`text-pending` (reuse existing status tokens; no new aliases needed)
- `src/components/transaction/TransactionListControls.tsx` — use `Spinner` primitive
- All components using `rounded-lg`/`rounded-md`/`rounded-full` — replace with `rounded-card`/`rounded-input`/`rounded-badge`

All existing tests must pass. `npm run check` must pass.

---

### Phase 4 — Safenet Brand Alignment (PR 4)

**Goal**: Align the explorer's visual identity with the Safenet staking UI so both products feel like part of the same family. Token names established in Phases 1–3 are preserved; only token **values**, fonts, shadow system, and the card visual style change.

**Design principles from the staking UI**:
- Cards have **no border radius** and share the **same background colour as the underlying layer** — depth comes from shadows, not borders or elevation
- Dark mode cards use a subtle green-tinted glow on hover instead of a visible border
- Monospace font (`Geist Mono Variable`) as the default sans, `Citerne` serif for headings
- Colours are hex-based (`#FFFFFF`, `#121312`, `#12FF80`), not oklch

**Token value updates** — names unchanged, values updated to match the Safenet brand palette:

| Token (name unchanged) | Light | Dark |
|---|---|---|
| `--color-surface-0` | `#FFFFFF` | `#121312` |
| `--color-surface-1` | `#FFFFFF` | `#1A1B1A` |
| `--color-title` | `#000000` | `#FFFFFF` |
| `--color-title-hover` | `#1A1B1A` | `#E0E0E0` |
| `--color-sub-title` | `#808B85` | `#A1A3A7` |
| `--color-muted` | `#CCD1CE` | `#1E201E` |
| `--color-surface-outline` | `rgba(0,0,0,0.12)` | `rgba(255,255,255,0.32)` |
| `--color-primary` | `#12FF80` | `#12FF80` |
| `--color-button` | `#000000` | `#12FF80` |
| `--color-button-hover` | `#1A1B1A` | `#0FDA6D` |
| `--color-button-content` | `#FFFFFF` | `#000000` |
| `--color-positive` | `#00B460` | `#27D18B` |
| `--color-error` | `#FF5F72` | `#FF5F72` |
| `--color-warning` | `#FF8061` | `#FF8061` |
| `--radius-card` | `0` | `0` |

**New tokens** (following existing naming convention):

| Token | Light | Dark | Purpose |
|---|---|---|---|
| `--color-info` | `#5FDDFF` | `#5FDDFF` | Informational highlights |
| `--color-secondary` | `#EFFFF4` | `#1E201E` | Subtle tinted surfaces |
| `--color-safe-light-green` | `#B0FFC9` | `#B0FFC9` | Brand tint |
| `--color-guardians-blue` | `#001F26` | `#001F26` | Deep brand dark |
| `--shadow-card` | `0 1px 3px 0 rgb(0 0 0/0.06), 0 1px 2px -1px rgb(0 0 0/0.06)` | `none` | Default card depth |
| `--shadow-card-hover` | `0 4px 12px -2px rgb(0 0 0/0.10), 0 2px 4px -2px rgb(0 0 0/0.06)` | `0 0 0 1px rgb(18 255 128/0.08)` | Card hover state |
| `--shadow-elevated` | `0 8px 24px -4px rgb(0 0 0/0.10), 0 4px 8px -4px rgb(0 0 0/0.06)` | `0 0 0 1px rgb(18 255 128/0.10), 0 4px 16px -4px rgb(0 0 0/0.40)` | Modals, popovers |

Map `--shadow-*` in `@theme` as `shadow-card`, `shadow-card-hover`, `shadow-elevated`.

**Typography changes**:
- Replace `@import` of Inter with `@fontsource-variable/geist-mono`
- Add `Citerne` `@font-face` declarations (woff2 files added to `src/assets/fonts/`)
- Update `--font-sans` → `Geist Mono Variable, ui-monospace, monospace`
- Add `--font-heading` → `Citerne, Georgia, serif`

**Card design changes** (component updates to match zero-radius, same-background style):
- `Box` component: remove `rounded-card` and `border border-surface-outline`; add `shadow-card` with `hover:shadow-card-hover transition-shadow`
- `--radius-card` is already the token controlling card radius; setting it to `0` in `styles.css` is the only change needed for all cards — no component edits beyond `Box` itself

Files touched:
- `src/styles.css` — update token values, add shadow tokens, new font imports
- `src/assets/fonts/` — add `Citerne-Regular.woff2`, `Citerne-Medium.woff2`
- `src/components/Groups.tsx` — `Box`: remove border and radius classes, add shadow classes
- `package.json` — add `@fontsource-variable/geist-mono` dependency

No token names change. No other component files require edits. All existing tests must pass. `npm run check` must pass.

