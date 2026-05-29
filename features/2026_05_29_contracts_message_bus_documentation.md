# Feature Proposal: Document Contracts as Message Bus and Signing Precedence
Component: `contracts`

---

## Overview

Two closely related design principles need to be formally documented across the contracts and technical documentation:

1. **`FROSTCoordinator` is a message bus** — it sequences and authenticates validator actions but does not decide ceremony outcomes. The primary source of truth is the FROST cryptographic math: a signing ceremony succeeds if and only if a valid threshold Schnorr signature is assembled and verifiable on-chain.

2. **Signing takes precedence over declining** — `signDecline` is purely indicative. If a participant both declines and signs in the same ceremony, the signature share (`SignShared`) takes precedence and the decline is disregarded. This must be documented at the contract level so clients consuming on-chain events apply the correct interpretation.

Both principles emerged from the explicit transaction rejection design and were confirmed during protocol review. This PR captures them as durable documentation in the contracts README, contract NatSpec, and the technical overview.

---

## Architecture Decision

### The coordinator is a message bus, not a decision-maker

On-chain events (`SignRevealedNonces`, `SignDeclined`, `SignShared`, `SignCompleted`, etc.) are coordination signals — they sequence validator actions and provide a globally ordered, immutable log. They do not themselves constitute outcomes.

The authoritative result of any signing ceremony is the FROST Schnorr signature: a cryptographic proof that a threshold of honest participants agreed. This is verifiable independently of any on-chain state by anyone with the group public key and the signed message. On-chain state is a projection of this cryptographic truth, not the source of it.

This principle directly motivates why `signDecline` makes no state changes: on a chain that experiences reorgs, a state-changing decline could retroactively alter coordinator state in a way that creates liveness issues (e.g., replacing a `signRevealNonces` with a `signDecline` after a reorg, leaving validators with no recovery path). Since the coordinator is only a message bus, the conservative and correct approach is to record the decline as an event and let the cryptographic outcome speak for itself.

### Signing takes precedence over declining

A `SignDeclined` event is an informational signal from a validator that it does not intend to participate. Because it makes no state changes, nothing prevents a participant from later submitting a `signShare` for the same ceremony (or vice versa). The intended interpretation — enforced by clients — is:

- If a participant has a `SignShared` event for a ceremony, they are considered to have **participated**, regardless of any `SignDeclined` event.
- A `SignDeclined` event is only meaningful if the participant has no corresponding `SignShared` event.

This rule is already implemented in the explorer's `getDeclined` logic and must be documented at the contract level so that future clients apply the same interpretation.

---

## Tech Specs

### Files changed

| File | Change |
|---|---|
| `contracts/README.md` | Add "Design Philosophy" section |
| `contracts/src/FROSTCoordinator.sol` | Expand contract-level `@notice` NatSpec; update `SignDeclined` event NatSpec; expand `signDecline` NatSpec |
| `docs/overview.md` | Expand `FROSTCoordinator Contract` section; add signing precedence note to Sign section |

---

## Implementation Phases

Single PR — all changes are documentation only, no logic changes.

---

## Open Questions / Assumptions

- Reorg hardening (forking in the guard vs. permissionless ceremony restart) is tracked separately and is not in scope for this PR.
- Phase 2 of explicit transaction rejection (`signDeclineWithCallback`) may introduce state changes once reorg safety guarantees are better understood. This documentation should be revisited at that point.
