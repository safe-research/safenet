---
title: Introduction
description: Shieldnet enforces transaction security onchain by preventing high-risk transactions from executing.
---

Shieldnet is a protocol for **onchain transaction security enforcement**.

It acts as a **last line of defense** against malicious or high-risk transactions by ensuring that transactions are **validated before execution**, rather than merely displaying non-binding warnings.

Shieldnet replaces centralized transaction-checking services with a **resilient, validator-based network** that enforces security guarantees onchain.

## Core problem

Transaction security remains an unsolved problem in Web3:

- Users are forced to **blind-sign** opaque transactions
- Wallet UIs can be compromised or misleading
- One malicious signature can lead to **irreversible loss**
- Proper operational security is hard, and failure probability increases over time

Even simple onchain actions (e.g. approvals, signer changes) can be embedded in deceptive transaction flows.

## Goals and value proposition

Shieldnet is designed to:

- **Prevent malicious transactions from executing**, not just warn about them
- Make transaction security **accessible to all users**, not only advanced or institutional ones
- Provide **additive security** that complements existing wallets and protocols
- Create **public, accountable security guarantees**, enforced onchain

Unlike offchain checkers, Shieldnet produces **binding enforcement**, not advisory signals.

## What makes Shieldnet different

Shieldnet’s core differentiators are:

- **Onchain enforcement**  
  Transactions deemed malicious are prevented from executing.

- **Decentralized validation**  
  Security checks are performed by a network of validators, not a single API or server.

- **Public accountability**  
  Validator attestations are onchain and auditable, increasing trust and reliability.

- **Open integration**  
  Any wallet, protocol, or transaction-checking service can integrate with Shieldnet.

## Who is Shieldnet for?

### Wallet operators
Integrate Shieldnet to protect users from malicious transactions without relying on centralized infrastructure.

→ See: [Integration → Wallets](/docs/integration/wallets)

### Node operators / validators
Participate in transaction validation and enforcement by running a Shieldnet validator.

→ See: [Operators → Validator overview](/docs/operators/overview)

### Stakers and delegators
Delegate stake to validators and earn rewards for securing the network.

→ See: [Protocol → Staking](/docs/protocol/staking)

### Transaction checkers and security firms
Provide detection logic and risk signals that can be enforced onchain via Shieldnet.

→ See: [Integration → Transaction checkers](/docs/integration/tx_checkers)

## Scope, limitations, and outlook

Shieldnet v1 focuses on:

- **Safe transactions on EVM chains**
- **Permissioned validator set**
- **Pre-defined security security checks**
- **Onchain enforcement via Safe guards**

This is a deliberate starting point. Transaction security is complex, and Shieldnet prioritizes correctness, reliability, and measurable guarantees in its initial deployment.

As a next step, onchain enforcement will be enabled including more advanced security checks, and transaction checker competition.

→ See: [Roadmap](/docs/roadmap)
