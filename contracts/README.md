# Contracts

This folder contains the smart contracts for Safenet.

## Design Philosophy

### Contracts as a message bus

The `FROSTCoordinator` and `Consensus` contracts act as a **coordination message bus**, not as decision-making systems. On-chain events sequence validator actions and provide a globally ordered, immutable communication log. They do not themselves constitute ceremony outcomes.

The **primary source of truth is the FROST cryptographic math**: a signing ceremony succeeds if and only if a valid threshold Schnorr signature is assembled and verifiable on-chain. This signature is independently verifiable by anyone with the group public key and the signed message — no trust in on-chain state is required.

This principle has concrete implications for contract design:

- Functions that record validator intent (e.g., `signDecline`) make no state changes. Recording an event is sufficient; the cryptographic outcome speaks for itself.
- On a chain that can experience reorgs, state-changing side effects of coordination signals could create liveness issues. Keeping the coordinator as a pure message bus is the conservative and correct default.

### Signing takes precedence over declining

`signDecline` is purely indicative. If a participant emits both `SignDeclined` and `SignShared` for the same signing ceremony, the signature share (`SignShared`) takes precedence and the decline is disregarded. Clients consuming on-chain events must apply this interpretation: a participant is considered to have declined only if they have a `SignDeclined` event and no corresponding `SignShared` event for the same ceremony.

## Audits

See [audits/audit.md](./audits/audit.md) for audit reports.
