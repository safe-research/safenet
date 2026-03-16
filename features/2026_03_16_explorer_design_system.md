# Feature Proposal: Explorer Design System
Component: `explorer`

---

## Overview

The explorer already has a solid foundation: Tailwind CSS 4 with CSS custom properties as design tokens, a `cn()` utility, and domain-organized components. However, the token layer is incomplete (missing `muted` text, no formal radius scale, no spacing aliases), a handful of components bypass the token system with raw Tailwind colours (`text-red-500`, `text-green-500`, `text-yellow-500`), and repeated Tailwind class strings are copy-pasted rather than encapsulated in primitives.

This specification formalises the existing approach — no new frameworks — into a proper design system:

1. **Phase 1 (PR 1)**: Audit and complete the token layer in `styles.css`
2. **Phase 2 (PR 2)**: Establish primitive component layer (`Button`, `Input`, `Label`, `Badge` variants)
3. **Phase 3 (PR 3)**: Standardise composite components — eliminate hardcoded colour bypasses and unify repeated class patterns

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
- `--color-keygen-error` / `--color-keygen-success` / `--color-keygen-pending` — semantic aliases for `red-500`, `green-500`, `yellow-500` used in `KeyGenStatusItem`
- Fix misleading comment on `--color-surface-1` in light mode (value is `oklch(1 0 0)` = white, not "black")

**Radius scale to formalise** (currently `rounded-lg`, `rounded-md`, `rounded-full` are used ad-hoc):

| Token | Value | Usage |
|---|---|---|
| `--radius-sm` | `0.375rem` | Inputs, inline chips |
| `--radius-md` | `0.5rem` | Cards (Box), dropdowns |
| `--radius-full` | `9999px` | Badges, avatars |

Map these into `@theme` so that `rounded-sm`, `rounded-md`, `rounded-full` resolve to the design system values, not Tailwind defaults.

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
| `src/styles.css` | Add missing tokens, fix comment, add radius scale, add `--color-muted`, add keygen semantic colour tokens |
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

**Radii** (new, in `@theme`):

```
--radius-sm   → rounded-sm
--radius-md   → rounded-md   (override Tailwind default)
--radius-full → rounded-full (keep Tailwind default value)
```

**Typography** (additions to `@theme`):

```
--text-2xs     (already defined)
--font-mono    (new explicit alias)
```

### Button Variants

| Variant | Base classes |
|---|---|
| `primary` | `bg-button hover:bg-button-hover text-button-content rounded-sm px-4 py-2` |
| `ghost` | `text-sub-title hover:text-title hover:underline` |
| `icon` | `inline-flex items-center text-xs px-1.5 py-0.5 border border-surface-outline rounded-sm` |

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
1. Add `--color-muted` (light: `oklch(44.6% 0.03 256.802)`, dark: `oklch(63% 0.02 256.802)`) and map in `@theme`
2. Fix the comment on `--color-surface-1` in light mode (says "black", value is white)
4. Add `--radius-sm`, `--radius-md` in `@theme` (override Tailwind defaults to match current ad-hoc usage)
5. Add `--font-mono` in `@theme`
6. Add a comment block above each token group (Surfaces, Text, Button, Status, Radius, Typography) for discoverability

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
- `src/components/common/NetworkBadge.tsx` — replace `#4B5563` with `text-muted`/`bg-muted`
- `src/components/epoch/KeyGenStatusItem.tsx` — replace `text-red-500`/`text-green-500`/`text-yellow-500` with `text-error`/`text-positive`/`text-pending`
- `src/components/transaction/TransactionListControls.tsx` — use `Spinner` primitive

All existing tests must pass. `npm run check` must pass.

---

## Open Questions / Assumptions

1. **`--color-surface-1` light mode value**: ✅ Fix comment only — value `oklch(1 0 0)` is correct (white), the comment erroneously says "black".

2. **`--radius-md` override**: ⚠️ **Open** — Tailwind's default `rounded-md` is `0.375rem`; the app currently uses `rounded-lg` (`0.5rem`) for cards and `rounded-md` for inputs. Remapping `--radius-md` to `0.5rem` would make `rounded-md` mean "card radius". Needs confirmation before Phase 1 lands.

3. **`text-muted` vs `text-sub-title`**: ✅ They remain separate — `text-sub-title` for structural secondary text, `text-muted` for decorative/helper text.

4. **Keygen status colours**: ✅ Reuse existing status tokens — `text-error`, `text-positive`, `text-pending` replace `text-red-500`, `text-green-500`, `text-yellow-500` directly. No new aliases needed.

5. **Storybook / component catalogue**: ✅ Out of scope for now. May be revisited as a future feature (framework-free option: an in-app `/dev/components` route).
