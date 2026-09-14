# F-ENG-039 `BaseChecker`'s Article IV Part A allow-lists are materially wider than the Charter's R-4.1/R-4.2 exceptions, so owner, threshold, guard, module and singleton changes are never denied

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | sentinel-engine, checkers/base.rs |
| Location | crates/sentinel-engine/src/checkers/base.rs:13-30, :103-147, :151-193, :205-213 |
| Severity | Medium / Medium |
| Certainty | 80% (Critic C-ENG-B; E2 ceiling, read-only run) |
| Assumptions involved | A2, A3, A15 |
| Tags | verdict-policy, charter |

## Claim

R-4.1 and R-4.2 are the Charter's **deterministic** rules — "The Council does not exercise discretion. Failing any deterministic rule makes the transaction insecure." R-4.1's allowed-exception list has exactly two entries: `disableModule` with any valid parameter, and `setFallbackHandler` with the **zero address**; and every exception additionally requires the transaction to be **non-batched** with `value == 0`. R-4.2's only exception is a delegatecall that touches nothing but the `signedMessages` mapping.

`BaseChecker` implements a much wider allow-list and returns `Ok()` → `Verdict::Abstain` for all of it:

| Engine allows (self-call, `to == safe`) | Charter R-4.1 exception? |
| --- | --- |
| `addOwnerWithThreshold` (any arguments) | no — owner list and threshold |
| `removeOwner` (any arguments) | no — owner list and threshold |
| `swapOwner` (any arguments) | no — owner list |
| `changeThreshold` (any arguments) | no — signing threshold |
| `disableModule` (any arguments) | **yes** |
| `setFallbackHandler(h)` for `h` in a 7-address list | only for `h == address(0)` |
| `setGuard(0)` — removes the Safenet Guard itself | no |
| `enableModule(m)` for two specific `m` | no — and modules bypass the guard entirely |
| `setModuleGuard(0)` | no |
| any of the above with `value != 0` | no — the exception requires `value == 0` |
| any of the above inside a MultiSend batch | no — the exception requires "non-batched" |

| Engine allows (delegatecall) | Charter R-4.2 exception? |
| --- | --- |
| `migrateSingleton` / `migrateWithFallbackHandler` / `migrateL2Singleton` / `migrateL2WithFallbackHandler` on two migration contracts | no — these write the singleton slot and the fallback-handler slot, neither of which is `signedMessages` |
| `signMessage` on four sign-message libraries | **yes** — `signedMessages` only |
| `performCreate` / `performCreate2` on four CreateCall contracts | arguably yes — `CREATE` from the Safe's context writes no Safe storage slot (class I: `CreateCall.sol` is not in this checkout) |

The net effect is that this engine never emits the R-4.1 or R-4.2 denial the Charter mandates for the whole settings-change family; it abstains, and the sentinel then drops the request without voting (`crates/sentinel/src/service.rs:173-179`). The engine's failure mode is safe (no vote, so no attestation and no slashing exposure), which is why this is Medium rather than higher — but the rule family the crate exists to enforce is, for those shapes, not enforced at all, and an operator reading `{"verdict":"abstain"}` cannot distinguish "the engine has no opinion" from "the engine decided this settings change is fine".

Two secondary defects sit inside the same code:

- The no-argument entries match on the **selector prefix only** (`starts_with`), so calldata consisting of the selector plus arbitrary or truncated bytes is allowed. The Charter's exception requires "`data` is a valid setting-change function following canonical Solidity ABI encoding".
- `check_calls` never requires `value == 0` for a self-call; `base.rs:350-364` pins that as intended behaviour.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The self-call allow-list admits five settings functions on selector prefix alone, plus four argument-validated ones | E2 | crates/sentinel-engine/src/checkers/base.rs:103-147 | Q1 |
| 2 | The allow-listed guard/module/fallback-handler values | E2 | crates/sentinel-engine/src/checkers/base.rs:13-30 | Q2 |
| 3 | Passing the allow-list means `Abstain`, not a denial | E2 | crates/sentinel-engine/src/checkers/base.rs:41-46, :52-56 | Q3 |
| 4 | The same allow-list is applied to every sub-call of a MultiSend batch, so batched settings changes pass | E2 | crates/sentinel-engine/src/checkers/base.rs:205-213 | Q4 |
| 5 | A batch of owner changes is pinned as allowed by the crate's own tests | E2 | crates/sentinel-engine/src/checkers/base.rs:432-437 | Q5 |
| 6 | A self-call with non-zero `value` is pinned as allowed | E2 | crates/sentinel-engine/src/checkers/base.rs:349-355 | Q6 |
| 7 | Delegatecall migrations (which write the singleton slot) are allowed | E2 | crates/sentinel-engine/src/checkers/base.rs:156-169 | Q7 |
| 8 | Charter R-4.1's exception list and its non-batched/`value == 0` requirements | I (Charter text) | safenet-charter@44a1e53:Safenet_Arbitration_Charter.md:500-528 | Q8 |
| 9 | Charter R-4.2's only exception is `signedMessages`, and Part A is non-discretionary | I (Charter text) | safenet-charter@44a1e53:Safenet_Arbitration_Charter.md:546-556, :490-496 | Q9 |
| 10 | An engine abstention makes the sentinel drop the request without voting | E2 | crates/sentinel/src/service.rs:173-179 | Q10 |

**Q1** `crates/sentinel-engine/src/checkers/base.rs:103-117` (the `disableModule` arm at `:120-122` follows the same shape; the four argument-validated arms at `:125-144` are shown at Q2 by their allow-lists)

```rust
fn check_self_calls(tx: &SafeTransaction) -> bool {
    // No-arg checks: any calldata starting with the right selector is allowed.
    if tx
        .data
        .starts_with(&safe::addOwnerWithThresholdCall::SELECTOR)
    {
        return true;
    }
    if tx.data.starts_with(&safe::removeOwnerCall::SELECTOR) {
        return true;
    }
    if tx.data.starts_with(&safe::swapOwnerCall::SELECTOR) {
        return true;
    }
    if tx.data.starts_with(&safe::changeThresholdCall::SELECTOR) {
```

**Q2** `crates/sentinel-engine/src/checkers/base.rs:13-21` and `:23-30`

```rust
const SUPPORTED_FALLBACK_HANDLERS: &[Address] = &[
    Address::ZERO,
    address!("85a8ca358D388530ad0fB95D0cb89Dd44Fc242c3"),
    address!("2f55e8b20D0B9FEFA187AA7d00B6Cbe563605bF5"),
    address!("3EfCBb83A4A7AfcB4F68D501E2c2203a38be77f4"),
    address!("fd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99"),
    address!("f48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4"),
    address!("017062a1dE2FE6b99BE3d9d37841FeD19F573804"),
];
```

```rust
const SUPPORTED_GUARDS: &[Address] = &[Address::ZERO];

const SUPPORTED_MODULES: &[Address] = &[
    address!("691f59471Bfd2B7d639DCF74671a2d648ED1E331"),
    address!("4Aa5Bf7D840aC607cb5BD3249e6Af6FC86C04897"),
];

const SUPPORTED_MODULE_GUARDS: &[Address] = &[Address::ZERO];
```

**Q3** `crates/sentinel-engine/src/checkers/base.rs:41-46` and `:52-56`

```rust
    async fn check(&self, transaction: &SafeTransaction, _context: &CheckContext) -> Verdict {
        match check_transaction(transaction) {
            Ok() => Verdict::Abstain,
            Err(rule) => Verdict::Insecure { rule },
        }
    }
```

```rust
pub fn check_transaction(tx: &SafeTransaction) -> Result<, RuleId> {
    check_settings_change(tx)
        .or_else(|| check_delegatecall_integrity(tx))
        .unwrap_or(Err(RuleId::R4_1SettingsChange))
}
```

**Q4** `crates/sentinel-engine/src/checkers/base.rs:205-213`

```rust
fn check_multi_send(tx: &SafeTransaction) -> bool {
    let Some((sub_txs, allows_delegate_calls)) = decode_multi_send_call(tx) else {
        return false;
    };

    sub_txs.iter.all(|sub_tx| {
        check_calls(sub_tx) || (allows_delegate_calls && check_delegate_calls(sub_tx))
    })
}
```

**Q5** `crates/sentinel-engine/src/checkers/base.rs:432-437`

```rust
    #[test]
    fn allows_multisend_with_multiple_owner_changes {
        assert!(
            check_transaction(&tx(
                address!("81a45AA50195f0A752159d5198780cDfb8e19732"),
                address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D"),
```

**Q6** `crates/sentinel-engine/src/checkers/base.rs:349-355`

```rust
    #[test]
    fn allows_self_call_with_nonzero_value {
        assert!(
            check_transaction(&tx(
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
                U256::from(1u64),
```

**Q7** `crates/sentinel-engine/src/checkers/base.rs:156-169`

```rust
    const MIGRATION_CONTRACTS: &[Address] = &[
        address!("6439e7ABD8Bb915A5263094784C5CF561c4172AC"),
        address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
    ];
    if MIGRATION_CONTRACTS.contains(&tx.to) {
        return tx.data.starts_with(&safe::migrateSingletonCall::SELECTOR)
            || tx
                .data
                .starts_with(&safe::migrateWithFallbackHandlerCall::SELECTOR)
            || tx.data.starts_with(&safe::migrateL2SingletonCall::SELECTOR)
            || tx
                .data
                .starts_with(&safe::migrateL2WithFallbackHandlerCall::SELECTOR);
    }
```

**Q8** `safenet-charter@44a1e53:Safenet_Arbitration_Charter.md:500-528` (extract)

```text
### R-4.1 — Settings-change blocking

#### Rule

- A transaction is insecure if it modifies any Safe setting (§ 2.10), unless it falls within an allowed exception below.
```

```text
#### Allowed exceptions

A settings change is not insecure under this rule only if all of the following apply:

- transaction is a non-batched multisignature Safe transaction;
- the transaction is triggered via `execTransaction`;
- `to` is the Safe itself;
- `operation` is `0` for CALL;
- `value` is `0`;
- `data` is a valid setting-change function following canonical Solidity ABI encoding, as interpreted under § 2.2 Solidity;
- the setting-change function is one of:
  - `disableModule` with any valid parameter;
  - `setFallbackHandler` with the zero address as the `handler` parameter.
```

**Q9** `safenet-charter@44a1e53:Safenet_Arbitration_Charter.md:546-556` and `:490-496`

```text
### R-4.2 — Delegatecall integrity

#### Rule

- A transaction is insecure if it performs a delegatecall that changes any storage slot of the Safe.

#### Exceptions

- Storage slots related to onchain message signing, meaning only data stored in the signedMessages mapping type and accessible via the Solidity-generated getter method, as interpreted under § 2.2 Solidity.
```

```text
#### Nature of deterministic rules

- The Council does not exercise discretion.
- Failing any deterministic rule makes the transaction insecure.
- No further analysis is required for that deterministic failure.
```

**Q10** `crates/sentinel/src/service.rs:173-179`

```rust
        let (approve, reason) = match outcome {
            CheckOutcome::Approved => (true, String::new()),
            CheckOutcome::Denied(rule) => (false, rule.to_string),
            CheckOutcome::Unknown => {
                tracing::warn!(%request_id, "engine check failed; dropping request unanswered");
                return (state, Vec::new());
            }
        };
```

## Trigger

Each of the following returns `{"verdict":"abstain"}` where the Charter mandates `{"verdict":"insecure","rule":"R-4.1"}` (or `R-4.2` for the last one). All are single requests with `gasPrice: "0x0"` so no other checker intervenes.

1. **Owner takeover.** `to = safe`, `operation = 0`, `value = "0x0"`, `data = swapOwner(prevOwner, oldOwner, <attacker>)` (selector `0xe318b52b`). This is `base.rs:332-347`'s own `allows_owner_change` vector.
2. **Threshold reduction to one.** `data = changeThreshold(1)` (selector `0x694e80c3`).
3. **Guard removal.** `data = setGuard(0x0000000000000000000000000000000000000000)` (selector `0xe19a9dd9`) — removes the SafenetGuard, after which the Safe is outside Safenet entirely.
4. **Module enablement.** `data = enableModule(0x691f59471Bfd2B7d639DCF74671a2d648ED1E331)` — module executions bypass the guard (`contracts/src/guard/SafenetGuard.sol:26-27`).
5. **Non-zero value.** Any of the above with `value = "0x1"` — the Charter exception requires `value == 0`.
6. **Batched.** Any of the above wrapped in `multiSend` to a canonical deployment with `operation = 1` — the Charter exception requires a non-batched transaction. `base.rs:432-464` pins this as allowed.
7. **Garbage arguments.** `data = <swapOwner selector> || 0x00` (5 bytes total) — accepted by `starts_with`, though it is not canonical ABI encoding.
8. **Singleton migration (R-4.2).** `to = 0x526643F69b81B008F46d95CD5ced5eC0edFFDaC6`, `operation = 1`, `data = 0xed007fc6` (`migrateSingleton`) — `base.rs:466-478`'s `allows_singleton_upgrade` vector.

Corpus shape: eight request/response pairs, each with the currently-observed `abstain` recorded, so that a decision to align with the Charter flips them to `insecure` in one visible diff. Controls that must stay `insecure R-4.1` today: `disableModule` is _allowed_ by both, so use an unknown self-call selector (`base.rs:277-295`) as the negative control.

## Considered and rejected

- **This is not exploitable, so it is not a finding.** Correct that the failure mode is safe: `Abstain` becomes `CheckOutcome::Unknown` and the sentinel drops the request without voting (Q10), so no attestation and no slashing exposure follow. It is filed as a correctness/compliance divergence on the crate's central rule family, at Medium, not as a fund-loss path.
- **The allow-lists are a deliberate product decision.** They may well be — every allow-listed address is a canonical Safe deployment, and the intent is clearly to permit routine Safe administration. But the Charter is the document `engine/rule.rs`'s codes cite, its Part A is explicitly non-discretionary (Q9), and §5.1 says precedent "cannot amend or override this Charter" while §5.4 reserves amendment to SafeDAO. Either the allow-lists narrow to the Charter or the Charter is amended; the divergence itself is the finding.
- **The Charter version in use might be older or newer than this one.** The available text (commit `44a1e53`) defines exactly the six rule codes `rule.rs:105-114` emits, with no others, which is the strongest available evidence that it is the matching version. Recorded as an assumption (A15).
- **`disableModule` and `signMessage` are also over-permissive.** They are not: `disableModule` with any parameter and a `signedMessages`-only delegatecall are precisely the Charter's two exceptions.
- **`performCreate`/`performCreate2` violate R-4.2.** Probably not — a `CREATE` executed in the Safe's context writes no Safe storage slot. Marked class I because `CreateCall.sol` is not in this checkout; it is excluded from the claim above rather than asserted either way.
- **`setGuard(0)` is the intended escape hatch.** The intended escape hatch is `announceTransaction` / `cancelAnnouncement` (Charter §2.18, `SafenetGuard.sol:355-368`), which is a different mechanism; nothing in the Charter carves out guard removal.

## Remediation options

1. Align `check_self_calls` with R-4.1: allow only `disableModule` and `setFallbackHandler(address(0))`, additionally requiring `value == 0`, a canonical ABI decode (replace `starts_with` with `abi_decode`), and not-batched (i.e. have `check_multi_send` refuse any self-call sub-transaction). Align `check_delegate_calls` with R-4.2 by dropping the migration contracts. Maximal Charter fidelity; will deny Safe administration flows that operators may rely on today.
2. Keep the wider allow-list but stop calling it Article IV Part A: emit `Verdict::Abstain` with an explicit log for "allowed by local policy, not by the Charter", and drive the Charter-mandated denial from a separate, Charter-faithful checker so the two can be compared. Lets the divergence be measured before it is closed.
3. Raise the mismatch to SafeDAO under §5.4 and record the intended exception list in the Charter, then generate the allow-list from it.
4. Independently of the policy question, fix the two mechanical defects now: require `value == 0` for self-calls and replace every `starts_with(&…::SELECTOR)` in `check_self_calls`/`check_delegate_calls` with a full `abi_decode`, so malformed calldata cannot ride an allow-listed selector.

Tests to add: for each Trigger above, a test asserting whichever verdict the team decides on — the current suite (`base.rs:277-764`, 20 tests) asserts the permissive answer for triggers 1, 5, 6 and 8, so a Charter alignment is a visible test change rather than a silent one. No code is committed.

## Trail

- Reviewer R9: drafted, self-estimate 78%. (Not in the analysis file's hypothesis list — found by reading `base.rs` against the Charter's R-4.1/R-4.2 text, which was unavailable to the earlier pass. All cited lines re-read at commit 2893917. The impact bound — abstain means no vote — was verified in `crates/sentinel/src/service.rs`, outside this reviewer's scope, and is cited rather than assumed.)

## Critic (C-ENG-B)

### 1. Charter verification — read in full, and the table is accurate

I read R-4.1 and R-4.2 end to end rather than checking the quoted fragments. R9's tables are correct. `safenet-charter@44a1e53:Safenet_Arbitration_Charter.md:516-528`:

> #### Allowed exceptions
>
> A settings change is not insecure under this rule only if all of the following apply:
>
> - transaction is a non-batched multisignature Safe transaction;
> - the transaction is triggered via `execTransaction`;
> - `to` is the Safe itself;
> - `operation` is `0` for CALL;
> - `value` is `0`;
> - `data` is a valid setting-change function following canonical Solidity ABI encoding, as interpreted under § 2.2 Solidity;
> - the setting-change function is one of:
>   - `disableModule` with any valid parameter;
>   - `setFallbackHandler` with the zero address as the `handler` parameter.

and `:550-554`:

> - A transaction is insecure if it performs a delegatecall that changes any storage slot of the Safe.
>
> #### Exceptions
>
> - Storage slots related to onchain message signing, meaning only data stored in the signedMessages mapping type and accessible via the Solidity-generated getter method, as interpreted under § 2.2 Solidity.

Against the code: `check_self_calls` (`base.rs:103-147`) allows `addOwnerWithThreshold`, `removeOwner`, `swapOwner`, `changeThreshold` and `disableModule` on a bare `starts_with` selector test with **no argument decoding at all**, plus `setFallbackHandler` for any of seven addresses (`base.rs:13-21`), `setGuard`/ `setModuleGuard` for the zero address, and `enableModule` for two addresses. Only `disableModule` and `setFallbackHandler(0)` are Charter exceptions; `setFallbackHandler` to any of the other six is not, and neither is `setGuard(0)` — which removes the Safenet Guard itself. `check_calls` (`base.rs:87-99`) applies no `value` test, and `check_multi_send` (`base.rs:205-213`) applies the same per-sub-call test inside a batch, so both the "`value` is `0`" and "non-batched" conditions are unimplemented. On R-4.2, `MIGRATION_CONTRACTS` + `migrate*` writes the singleton and fallback-handler slots, neither of which is `signedMessages`; only `SIGN_MESSAGE_LIBS` + `signMessage` matches the exception. **All Supported.**

### 2. Severity — Medium confirmed, because the failure is fail-closed

I traced the consequence rather than assuming it. `BaseChecker` returns `Ok()` → `Verdict::Abstain` (`base.rs:41-46`) — it can never affirm. So a Charter-insecure settings change is not rated `secure`; it falls through the rest of the chain, which has no opinion on `addOwnerWithThreshold` calldata (`decode_target` returns `None` at `address_poisoning.rs:116-139`; `decode_target_effects` yields nothing), and the engine answers **`Abstain`**. Per `crates/sentinel/src/engine.rs:175` and `crates/sentinel/src/service.rs:176-179` the sentinel then casts no vote, no attestation is produced, and the Guard refuses the transaction. The owner is protected.

So the harm is that the Charter's _deterministic_ floor — R-4.1 and R-4.2 admit no Council discretion (`:441` "Deterministic rule failure is established by direct application of the rule") — is not enforced by the engine, which returns "no opinion" where the Charter requires "insecure". That is a real coverage and consensus-quality defect, and it means the engine's denial rate systematically under-reports R-4.1/R-4.2, but it is not a fund-loss path. §8's "missing validation with contained impact" is the right row. **Medium** confirmed. I considered Low and rejected it: the omitted set includes `setGuard(0)`, i.e. the transaction that disables Safenet on that Safe, which is too central to file as a robustness nit.

### 3. Overlap with F-ENG-001 and F-ENG-003 — the answer I was asked for

I read both of C-ENG-A's files. **Yes, these overlap, and substantially — but they are not one finding.**

- **F-ENG-039 (this file)** is the _behavioural_ defect: `checkers/base.rs`'s allow-lists are wider than the Charter's exception lists. One defect, one fix site (`base.rs:13-30`, `:103-147`, `:151-193`, `:205-213`), and it covers R-4.1 and R-4.2 together plus the two conditions neither of C-ENG-A's files reaches — the Charter's "non-batched" requirement (`:520`) and its "`value` is `0`" requirement (`:524`).
- **F-ENG-001 / F-ENG-003** are primarily _documentation_ defects in `engine/rule.rs:16-19` and `:20-23`: the `RuleId` doc comments state a rule the Charter does not contain. Each then reaches across into `base.rs` to show the behaviour matches the wrong doc — and both files say so explicitly in their own header ("behavioural half in `checkers/base.rs`, R9's scope").

**Canonical assignment.** For the behavioural claim, **F-ENG-039 should be canonical** — it is the complete statement, it is scoped to the file that must change, and it does not split one fix across two rule codes. F-ENG-001 and F-ENG-003 should be retained and narrowed to the `engine/rule.rs` doc-comment mismatch, which is a genuine and separate issue: the doc comments are what a future maintainer will implement against, so leaving them stating a non-existent rule guarantees the behaviour is re-introduced after any fix. That residue is Informational-to-Low on its own.

Per §6 of the Critic brief I am not merging or deleting anything, and I have appended a matching cross-reference note to F-ENG-001 and F-ENG-003. Whoever compiles the report should present them as one behavioural finding plus two documentation findings, not as three independent Charter violations — otherwise the same defect is counted three times.

### 4. Finding verdict

**Confirmed** (A15 is TRUE, so this verdict-policy finding may reach Confirmed, and the Charter text was read in full context as the brief requires). Certainty **80%**. Severity **Medium** (unchanged).

## QA (QA-ENG)

**Execution: Not attempted (no toolchain)** — `rust-audit/state/baseline.md` §1. **No PoC written.** This is the deliberate omission in this run's engine PoC set, and the reason should be recorded rather than inferred: **F-ENG-039 is canonical for the behavioural defect, and its remediation is a policy decision that has not been made.** A PoC's regression test has to assert an expected verdict, and here nobody yet knows whether the expected verdict is `insecure R-4.1` (option 1), `Abstain` with a policy log (option 2), or unchanged pending a Charter amendment (option 3). Writing one would encode a decision the team has not taken. The existing suite (`base.rs:277-764`, 20 tests) already pins today's permissive answers for triggers 1, 5, 6 and 8, so whichever way the decision goes, the change is visible in those tests — which is the property a PoC would otherwise supply.

**Certainty unchanged at 80%. Severity unchanged at Medium.**

**Canonical-file note.** Per the Critic, F-ENG-039 is canonical for the **behavioural** `BaseChecker` divergence, and F-ENG-001 and F-ENG-003 for the `RuleId` **doc comments** describing the same rules. I have kept that split in all three `## QA` sections: the doc fixes are shippable today at zero risk and are recorded there; the behaviour question is here.

### Remediation check

- **Option 1 (align both allow-lists with R-4.1/R-4.2) — sound as Charter fidelity, and it is the option the Charter text supports — but it is a product decision with a hard prerequisite.** It turns owner, threshold, guard, module and singleton changes into denials for every Safe using Safenet. `Charter:542`'s "protocol-defined settings-change path outside Safenet's standard transaction-security flow" is the escape hatch that makes it acceptable, and **nothing in this checkout establishes that the path exists.** Shipping option 1 before it does converts this finding into a mass wrong-denial incident — and this audit established that a good-faith denial overruled in arbitration is slashed (`contracts/src/libraries/SentinelOracleRequests.sol:301-304`; see F-ENG-042's `## QA`). **Verify the path exists first.**
- **Option 2 (keep the allow-list but stop calling it Article IV Part A) — sound, and the best interim position.** Emitting `Abstain` with an explicit "allowed by local policy, not by the Charter" log, and driving the Charter-faithful denial from a separate checker, lets the divergence be **measured** before it is closed. That measurement is exactly what the option 1 decision needs and does not have. One caution: two checkers reaching different conclusions about the same transaction interacts with F-ENG-044 — under the current first-wins combinator, whichever is registered earlier silently wins, so option 2 must be built as an observability path (log/metric only), not as a second voting checker, until the combinator is fixed.
- **Option 3 (raise the mismatch to SafeDAO under §5.4 and generate the allow-list from the Charter) — sound, and the only durable resolution**, since it makes the allow-list derived rather than asserted. Slowest; start it in parallel with option 2 rather than after it.
- **Option 4 (fix the two mechanical defects now) — sound, risk-free, and it should not wait for the policy decision.** Requiring `value == 0` on an exempt self-call and replacing every `starts_with(&…::SELECTOR)` with a full `abi_decode` have no legitimate traffic behind them and no Charter ambiguity — the Charter requires canonical ABI encoding, so a malformed payload riding an allow-listed selector is unambiguously wrong today. **Ship option 4 immediately, independently of everything else.** This is the same recommendation I record under F-ENG-001 option 3.
- **Test hook: exists and is good.** `BaseChecker` is pure and `base.rs:216-764` is a substantial suite. Nothing is missing except the cases the decision will dictate.
- **Where the fix belongs: the checker (behaviour) and, under option 3, the Charter-to-allow-list derivation.** The `RuleId` doc comments are F-ENG-001/003's.

## In-flight impact (AS-EPIC)

**Pertains to unmerged branches, not to `main`.** Assessed against the documentation-only PR #917 (`pr/reality-module-sef-veto-epic`, `epics/2026_09_14_reality_module_sef_veto.md`). **Effect: no code change, but the plan risks widening this finding.** The epic plans a Safe module enabled on the SafeDAO Safe via `enableModule`, and never asks whether that Safe is Safenet-protected. On `main`, a direct `enableModule` for an unlisted module is **denied**: `check_self_calls` returns `false` (`crates/sentinel-engine/src/checkers/base.rs:135-138`), `check_settings_change` maps that to `Err(RuleId::R4_1SettingsChange)` (`:60-68`), and `BaseChecker::check` returns `Insecure` (`:41-45`). The escape hatch runs before `BaseChecker` (`main.rs:58-60`) and affirms first under `F-ENG-044` — that is the Charter's intended R-4.1 settings-change path. **Adding the module to `SUPPORTED_MODULES` would flip the direct path from `Insecure` to `Abstain`**, silently removing the only denial and widening exactly the off-Charter allow-list this finding reports.
