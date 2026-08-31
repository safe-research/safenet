# Contracts

This folder contains the smart contracts for Safenet.

## Design Philosophy

### Contracts as a message bus

The `FROSTCoordinator` and `Consensus` contracts act as a **coordination message bus**, not as decision-making systems. Onchain events sequence validator actions and provide a globally ordered, immutable communication log. They do not themselves constitute ceremony outcomes.

The **primary source of truth is the FROST cryptographic math**: a signing ceremony succeeds if and only if a valid threshold Schnorr signature is assembled and verifiable onchain. This signature is independently verifiable by anyone with the group public key and the signed message; no additional trust in onchain state is required. This ensures that the produced FROST attestations are portable and can be used to verify any Safe transaction on any chain without additional cross-chain message passing requirements.

## Audits

See [audits/audit.md](./audits/audit.md) for audit reports.
