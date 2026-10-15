# F3-CORE-060 Resubmissions compound both fee fields by 10% per round with no ceiling

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | safenet-core, `tx/fees.rs`, `tx/types.rs`, `tx/mod.rs` |
| Location | `crates/core/src/tx/fees.rs:33-56` (related: `crates/core/src/tx/types.rs:116-141`, `crates/core/src/tx/mod.rs:207-225`, `crates/core/src/tx/mod.rs:291-304`, `crates/core/src/tx/mod.rs:391-414`, `crates/core/src/tx/storage.rs:376-415`, `crates/core/src/tx/config.rs:15-37`) |
| Severity | Low / Low |
| Certainty | 75% (CA3: evidence checked) |
| Assumptions involved | A1, A4 |
| Tags | fees, dos |

## Claim

Every resubmission builds the transaction with `bump(fresh_estimate, last_recorded_fees)`, which raises `maxFeePerGas` and `maxPriorityFeePerGas` each to at least 110% of the previous submission. The previous submission is itself the last bump, so the fees compound: after k rounds a transaction carries 1.1^k times its first tip. Nothing bounds the growth. `tx::Config` has no absolute fee limit, `priority_fee_cap_percentage` is applied to the fresh estimate only (the code says bumps can bypass it), and `resubmit_stale` rebroadcasts every unexecuted transaction every `blocks_before_resubmit` (default 2) blocks, whatever the reason it is not included.

With Gnosis 5 s blocks, a transaction that stays in the mempool without inclusion carries 9.8x its first tip after 48 blocks (4 minutes), 97x after 96 blocks (8 minutes) and 9,412x after 192 blocks (16 minutes), executed below. On Gnosis the tip is most of the price (the handbook says so), and because `maxFeePerGas` grows in step, the effective tip paid at inclusion is the full inflated `maxPriorityFeePerGas`. The only practical bound is the signer balance: a node rejects a replacement whose `gas * maxFeePerGas` exceeds it, which the queue logs as a plain failure without advancing the fee floor, so the last accepted version reserves roughly the whole balance. If it then lands, a single action spends `gas_used / gas_limit` of that, most of the balance, as tip.

The handbooks present the cap as protection "against runaway estimates" (`docs/validator-handbook.md:73`, `docs/sentinel-handbook.md:65`). It bounds the ratio of tip to max fee of a fresh estimate; it does not bound the absolute fee of a resubmission.

I found no attacker-controlled way to keep a transaction accepted and unincluded under A2 and A4, so this is a missing fund-safety bound with a conditional trigger, not an exploit.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Each bump is computed from the previous submission's fees, so it compounds | E1 | `crates/core/src/tx/fees.rs:53-56` | `let bumped = previous.saturating_add(previous.div_ceil(10));` |
| 2 | A rebuilt transaction bumps the fresh estimate against the fees last recorded for it | E2 | `crates/core/src/tx/types.rs:119-120` | `let fees = fees::bump(estimate, self.fees());` |
| 3 | Every unexecuted row last submitted at or before `latest - blocks_before_resubmit` is rebroadcast, with no limit on rounds | E2 | `crates/core/src/tx/mod.rs:291-304`, `crates/core/src/tx/storage.rs:400-401` | `AND (n.submitted_at IS NULL OR n.submitted_at <= ?)` |
| 4 | The priority fee cap applies to the fresh estimate before the bump, and bumps can exceed it | E2 | `crates/core/src/tx/mod.rs:396-398`, `crates/core/src/tx/fees.rs:38` | `/// Note that fee bumps can cause priority fee caps to not be observed.` |
| 5 | The queue configuration has no absolute fee limit | E2 | `crates/core/src/tx/config.rs:15-25` | `pub priority_fee_cap_percentage: Option<f64>,` |
| 6 | A failed send (for example insufficient funds) does not advance the floor, so the last accepted version is the largest affordable one | E2 | `crates/core/src/tx/mod.rs:351-360` | `"submission failed, will retry without bumping fees"` |
| 7 | Executed growth through the real `update_block_status` path: 9.8x the first tip after 24 rounds, 97.0x after 48 | E1 | `rust-audit/state/run3/logs/R3-fee-config-probe.txt` (section E1) | `round 48 (block 106): max_fee 105166681684 tip 97017233998 = 97.0x first tip` |
| 8 | On Gnosis the tip makes up most of the price | I | `docs/validator-handbook.md:73` | `On Gnosis Chain, the base fee is very low relative to the priority fee, so the priority fee makes up the bulk of gas costs.` |

Basis quotes in full:

Row 1, `crates/core/src/tx/fees.rs:39-56`:

```rust
pub fn bump(fresh: Eip1559Estimation, previous: Option<Eip1559Estimation>) -> Eip1559Estimation {
    let Some(previous) = previous else {
        return fresh;
    };
    Eip1559Estimation {
        max_fee_per_gas: bump_fee(fresh.max_fee_per_gas, previous.max_fee_per_gas),
        max_priority_fee_per_gas: bump_fee(
            fresh.max_priority_fee_per_gas,
            previous.max_priority_fee_per_gas,
        ),
    }
}

/// Returns `fresh`, raised to at least 10% above `previous`.
fn bump_fee(fresh: u128, previous: u128) -> u128 {
    let bumped = previous.saturating_add(previous.div_ceil(10));
    fresh.max(bumped)
}
```

Row 3, `crates/core/src/tx/mod.rs:291-304`:

```rust
    async fn resubmit_stale(&mut self, block: u64) -> Result<(), Error> {
        let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);
        let stale = self.storage.stale_submissions(submitted_before).await?;
        if stale.is_empty() {
            return Ok(());
        }

        for transaction in stale {
            tracing::debug!(nonce = transaction.nonce, "resubmitting stale transaction");
            self.submit_transaction(transaction, block).await?;
        }

        Ok(())
    }
```

Row 4, `crates/core/src/tx/mod.rs:395-398`:

```rust
                let fees = self.provider.estimate_eip1559_fees().await?;
                let fees = match self.config.priority_fee_cap_percentage {
                    Some(cap) => {
                        let capped = cap_priority_fee(fees, cap);
```

Row 6, `crates/core/src/tx/mod.rs:351-360`:

```rust
            // Other failures do not establish that the transaction reached the
            // mempool or that its fees were insufficient. Leave the last
            // accepted fee floor unchanged and retry without increasing it.
            Err(err) => {
                tracing::warn!(
                    nonce = submission.nonce,
                    ?err,
                    "submission failed, will retry without bumping fees"
                );
            }
```

Row 7, executed output (`R3-fee-config-probe.txt`, sections B and E1; estimate `2 * 0.042 gwei + tip`, tip arbitrary, results as multiples of it):

```text
round  24 (+ 48 blocks): tip             9849732693 =        9.8x first tip; gas_limit * max_fee           2669277560750000 wei
round  48 (+ 96 blocks): tip            97017233998 =       97.0x first tip; gas_limit * max_fee          26291670421000000 wei
round  96 (+192 blocks): tip          9412343672264 =     9412.3x first tip; gas_limit * max_fee        2550745135951500000 wei
round 48 (block 106): max_fee         105166681684 tip          97017233998 =     97.0x first tip
```

## Trigger

A transaction that a node accepts but the chain does not include for many blocks, with the signer account otherwise funded. The probe reproduces the queue side by holding the mocked account nonce at 0 for 48 rounds (96 blocks). Production states that give the same input, none of them attacker-controlled under A2 and A4:

1. Direct mode: transactions at nonces above a missing nonce sit in the mempool as future transactions. All of them are bumped every round although none can be included before the gap closes. A gap that outlives a resubmission round needs the lowest nonce to keep failing for a non-fee reason, or to be dropped from resubmission, for example by an execution marker set from an `eth_getProof` answer served by a sibling block (see the R3 coverage log, observation O3).
2. An endpoint that accepts but does not propagate, for example a load-balanced endpoint whose backend is partitioned from its peers while block reads still reach a healthy backend.
3. Block producers that skip the account for any reason.

The cost scales with how long the condition lasts: the tip doubles about every 7.3 rounds (73 s). From a first submission that reserves 1/10,000 of the balance, the reservation reaches the balance in about 13 doublings, 16 minutes.

## Considered and rejected

- Fresh estimates track a base fee rise, so ordinary congestion does not compound: `bump` takes `max(fresh, previous * 1.1)` and the fresh `2 * base + tip` dominates once the base fee jumps. Only a transaction that stays unincluded with adequate fees compounds.
- Failed sends do not compound since #686 (`80f747c`): the floor advances only after acceptance or an underpriced rejection (`crates/core/src/tx/mod.rs:333-360`). That fixed the outage case; the accepted-but-unincluded case remains.
- Batched mode keeps one transaction in flight (`crates/core/src/tx/config.rs:65-74`), so it has no nonce gaps among its own transactions; the propagation and producer cases still apply.
- Mempool balance checks stop growth at the balance; they do not prevent the inflated tip from being paid when the transaction lands.

## Remediation options

1. Stop bumping above an absolute ceiling: a `max_fee_per_gas` limit in `[transactions]`, or a multiple of the current fresh estimate. Once at the ceiling, rebroadcast at the same fees (or not at all) instead of raising them. Tradeoff: a ceiling set too low can delay inclusion during a real base-fee spike; tie it to the fresh estimate to avoid that.
2. Bump only the lowest unexecuted nonce. A higher nonce cannot be included before it, so bumping it only raises what it pays later. Higher nonces need a rebroadcast only when their own `maxFeePerGas` falls below the base fee.
3. Count rounds per transaction and raise a metric or warning after a few, so operators see a stuck transaction before it is expensive.

Tests to add: drive an unincluded transaction for 100 rounds and assert the recorded fees never exceed the ceiling; with a nonce gap, assert that only the lowest in-flight nonce is bumped.

Pre-existing at run 2's commit `fe9e84c`: yes. `tx/fees.rs` is unchanged since then, and the resubmission and fee cache logic were already in `tx/mod.rs` (`git show fe9e84c:crates/core/src/tx/fees.rs`).

## Trail

- 2026-10-08 Reviewer R3 (security): drafted. Mechanism E1 (probe through `update_block_status` and `AllocatedTransaction::build`, compiled against the audit worktree, no tracked file changed); production trigger conditional. Self-estimate 85% for the mechanism, 40% that a production trigger occurs at Aegis parameters.

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/tx/fees.rs:53-56`; E1 through the probe. |
| 2 | Supported | `crates/core/src/tx/types.rs:119-120`. |
| 3 | Supported | `crates/core/src/tx/mod.rs:291-304`; `crates/core/src/tx/storage.rs:400-401`. |
| 4 | Supported | `tx/mod.rs:396-398`; `fees.rs:38`. |
| 5 | Supported | `crates/core/src/tx/config.rs:15-25`. |
| 6 | Supported | `tx/mod.rs:351-360`. A failed send does not call `record_submission`, so `submitted_at` keeps the last accepted block and the row is stale again every round (`storage.rs:400-401`). |
| 7 | Supported | `rust-audit/state/run3/logs/R3-fee-config-probe.txt`, section E1. |
| 8 | Supported | `docs/validator-handbook.md:73`. |

Finding verdict: Plausible. Severity: Low to Low. Certainty: 50, band 40 to 69 (mechanism E1; no production trigger shown, and the reviewer found no attacker route under A2 and A4). Duplicate: canonical.

Confirmation, and an impact the file understates. The handbooks tell operators to fund the signer "only with the amount needed for gas" (`docs/validator-handbook.md:63`; the sentinel "gas and bonds", `docs/sentinel-handbook.md:57`) and put six months of validator gas at roughly $10 (`docs/validator-handbook.md:69`). So the balance bound is small, and reaching it also stops the service: every later transaction is rejected for insufficient funds, logged as a plain failure and retried at the same price (`tx/mod.rs:351-360`), and the rows fill the in-flight limit until someone refunds the signer. The impact is liveness as well as cost. Spec note: the cap keeps its ratio promise, because both fields grow by the same factor (up to rounding), so a bumped tip stays within the cap share of the bumped max fee; it does not bound the absolute fee, and the handbook's "protecting against runaway estimates" (`docs/validator-handbook.md:73`) is about estimates only. Related: R3 observation O3 (an executed marker set from a sibling block's `eth_getProof`, `tx/mod.rs:372-380`, `storage.rs:281-292`) is the most concrete route to a lasting Direct-mode nonce gap, the first trigger listed.

QA recipe (second reproduction, trigger side). Put a small loopback JSON-RPC proxy on a POC port in front of a local Anvil (interval mining, `--block-time 1`): it forwards every method except `eth_sendRawTransaction`, which it records and answers with the transaction hash without forwarding. That models trigger 2 (an endpoint that accepts but does not propagate). Run the real validator or sentinel queue in Direct mode against the proxy with one queued action. Expected: a resubmission every 2 blocks, each recorded raw transaction carrying `maxPriorityFeePerGas` 10% above the previous (decode with `cast decode-tx`). After 40 rounds, forward the last recorded transaction to Anvil: it lands with an `effectiveGasPrice` tip about 1.1^40 (45x) the first. Bite check: an absolute ceiling (remediation 1) keeps the recorded fees flat once reached.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced.

Route: the real `Driver` and `TransactionQueue` (Direct mode, default `blocks_before_resubmit` 2, no fee cap) follow a real Anvil chain (chain id 100, 1 s blocks) through `Provider::connect` over HTTP, with one 21,000-gas action queued at startup. The RPC URL is a loopback proxy that records every `eth_sendRawTransaction` and answers with its hash without forwarding it (trigger 2), and reports a 1 gwei tip in `eth_feeHistory` (Anvil's empty blocks would otherwise make alloy estimate a 1 wei tip). In 90 s the queue sends 45 versions of nonce 0, one every 2 blocks, each tip 10% above the previous: 2.59 times the first tip at version 10, 17.45 at version 30, 45.26 at version 40, 66.26 at version 44. Sent to Anvil, that last version lands with `effectiveGasPrice` 66,264,076,479 wei over a 258 wei base fee, a tip 66.3 times the first. At Gnosis's 5 s blocks, 44 rounds take about 7.3 minutes.

Command and output: `poc/F3-CORE-060/` (`fee-compounding-at-361aaf3.txt`, `swallowed-submissions-at-361aaf3.tsv`).

Bite check: remediation 1 as a ceiling of three times the fresh estimate in `bump_fee`: the tip reaches 3.00 times the first by version 12 and stays flat, and the landed version pays a 3.0 times tip. Sound as a cost bound. Trade-offs: a ceiling below what a real spike needs delays inclusion, and a resubmission at unchanged fees is the same transaction, which real nodes answer as already known; the queue then logs a failed send every round.

Certainty: 75 (mechanism and cost E1 end to end; the trigger is modelled by the proxy, and no production route to an accepted but never included transaction is shown, as the Critic noted).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | Nobody found. The trigger is a transaction that a node accepts and the chain does not include for minutes: a Direct-mode nonce gap that outlives a resubmission round, an endpoint that accepts without propagating, or producers that skip the account. The reviewer found no attacker route under A2 and A4 |
| Cost | n/a |
| Condition | One of those states lasting many 2-block rounds: 9.8x the first tip after 4 minutes, 97x after 8, roughly the whole signer balance after 16 |
| Gate | Gate 1 passes (end to end with a swallowing proxy). Gate 3: no unprivileged trigger. Gate 4: bounded by a deliberately small signer balance; the handbook sizes six months of validator gas at about 10 dollars (`docs/validator-handbook.md:69`) and tells operators to fund only that (`:63`). The larger harm is liveness: once the balance is reserved, later transactions fail for insufficient funds until someone refunds the signer |
| Practicality | conditional (an accepted transaction stays unincluded for several minutes) |
| Recommended severity | Low (reviewer, Critic and QA agree) |
| Intended route and parameters | Real `Driver` and `TransactionQueue` in Direct mode against real Anvil (chain id 100) through a recording proxy that never forwards `eth_sendRawTransaction`: the proxy is the modelled trigger, the fee path is real |
| Cheapest settling change | None needed for the severity |

Reason. On Gnosis, blocks are far from full and a correctly priced transaction is normally included at the next slot, so a transaction stuck for minutes points to an RPC or nonce fault that is itself rare. A small balance caps the money at stake; the ceiling is worth adding because the failure mode, a drained signer and a stalled service, needs a human to notice.

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Evidence holds. Final: Accepted, Low, certainty 75 |
| Evidence check | `poc/F3-CORE-060/fee-compounding-at-361aaf3.txt`: 45 swallowed submissions, the tip at 1.10x, 2.59x, 6.73x, 17.45x and 45.26x the first |
| Final severity | Low, agree with PC3; no route to an accepted but never included transaction is shown |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | CONFIRMS `F-CORE-060` for the accepted-resubmission arm only (run 2's `F2-CORE-060` arm) |
| Canonical | `F-CORE-060` |
| Ledger values | High, certainty 98 (the re-validation). This file's Low covers the accepted arm. The arm that carries the High is not in this file: an underpriced rejection writes `submitted_at = NULL`, so the row is rebuilt 10 % higher every block until the signer balance stops it (run 1 executed it against a real Anvil fee market). PC3 did not weigh that arm, so ledger section 7 lists the band as unsettled |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
