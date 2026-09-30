# PoC — F-VAL-001

**DKG encryption key `q` has no proof of possession: a participant that republishes a peer's `q` recovers that peer's complete FROST signing share while the group finalizes normally.**

> **This code has never been compiled or run.** There is no Rust toolchain on the audit host (`state/baseline.md` §1: `cargo`, `rustc`, `rustup` all absent, A9 FALSE). Every identifier, signature, field name and visibility used below was checked by reading `crates/validator/src/frost/{ecdh,keygen,marshal,participants}.rs` and `crates/validator/src/bindings.rs` at commit `2893917`, but mechanical fixes may still be needed. The one place a mechanical fix is most likely is flagged in §5.

## 1. Wiring it in

The attack needs three things that are not public outside `crate::frost`:

| Item | Visibility | Why it is needed |
| --- | --- | --- |
| `frost::marshal` | `mod marshal;` (private to `crate::frost`) | `frost_scalar` / `solidity_scalar` for the `U256` ↔ `k256::Scalar` conversions |
| `frost::participants` | `mod participants;` (private to `crate::frost`) | `identifier(address)` for the Lagrange x-coordinates |
| `keygen::KeyShare::as_key_package` | `pub(super)` | reading the victim's real signing share as ground truth |

So the PoC must be compiled **as a child module of `crate::frost`**, not as an integration test in `crates/validator/tests/`. Add exactly these three lines to the end of `crates/validator/src/frost/mod.rs` (above its existing `#[cfg(test)] mod tests`):

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-001/poc.rs"]
mod poc_f_val_001;
```

`#[path]` is resolved relative to the directory containing `mod.rs` (`crates/validator/src/frost/`), so the four `..` segments land on the repository root.

This is the _only_ edit to a tracked file, and it is confined to `#[cfg(test)]`. Revert it when done.

## 2. Commands

```sh
# all three tests
cargo test -p validator --bins frost::poc_f_val_001 -- --nocapture

# the cheapest and most decisive one on its own (~1 s)
cargo test -p validator --bins frost::poc_f_val_001::pad_opens_two_recipients_slots
```

## 3. Fixtures — spelled out

Under assumption A2 everything the attacker sends is attacker-chosen, so the inputs are literal:

| Role             | Address (Anvil default account)                   |
| ---------------- | ------------------------------------------------- |
| bystander `B₁`   | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` (#0) |
| bystander `B₂`   | `0x70997970C51812dc3A010C7d01b50e0d17dc79C8` (#1) |
| **victim `A`**   | `0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC` (#2) |
| bystander `B₃`   | `0x90F79bf6EB2c4f870365E785982E1f101E93b906` (#3) |
| bystander `B₄`   | `0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65` (#4) |
| bystander `B₅`   | `0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc` (#5) |
| **impostor `M`** | `0x976EA74026E726554dB657fA54763abd0C3a0aa9` (#6) |

`n = 7`, `threshold = group_threshold(7) = 7/2 + 1 = 4` (`crates/validator/src/consensus/group.rs:219-221`). One malicious participant out of seven is inside A2's `< n/3` bound. Polynomials and encryption keys are freshly sampled per run by `keygen::setup`; the attack does not depend on any particular value, which is the point — run it repeatedly if you want a statistical argument.

### The attacker's literal calldata

1. **`keyGenCommit(gid, poap_M, commitment_M)`** where `commitment_M = { q: <A's q, copied verbatim from A's KeyGenCommitted log>, c: <M's own c>, r: <M's own r>, mu: <M's own mu> }`. In the PoC this is the single mutation in `run_rounds_1_and_2`: `commitments.get_mut(&IMPOSTOR).q = copy_point(&commitments[&VICTIM].q)`. Onchain this is accepted because `FROSTCoordinator.sol:377` checks only `q != 0`.
2. **`keyGenComplain(gid, X)` × 6**, one per `X ∈ {B₁…B₅, A}`, sent **before** `M` publishes any share. Modelled in `harvest_pads`, which reads each accused's `reveal_secret_share(sharing_state_X, M)` — exactly the value `crates/validator/src/state/keygen.rs:734-745` queues as `Action::KeyGenComplaintResponse { secret_share, .. }`.
3. **`keyGenSecretShare(gid, share_M)`** where `share_M.y` is unchanged and `share_M.f[j] = f_M(id_{X_j}) XOR pad(sk_{X_j}, q_A)`, the pads coming from step 2. Built by `forge_impostor_share`. Slot order is ascending participant address excluding `M`, matching `crates/validator/src/frost/keygen.rs:196-214`.
4. **`keyGenConfirm(gid)`** from every participant including `M` — `M` is eligible because all six complaints it filed were answered (`FROSTParticipantMap.sol:219-222`).

Block numbers do not matter for the cryptographic core. They matter for the _onchain_ variant only, through the `CollectingShares` deadline set at `state/keygen.rs:373-379` from `key_gen_timeout` (default 120 blocks, `crates/validator/validator.sample.toml`); the six complaint round trips must land inside it. Per Critic C-VAL-A's Correction 2, in the **genesis** ceremony that deadline is `None` (`state/keygen.rs:52-53`), so the window is unbounded there.

## 4. What a pass and a failure mean

| Test | PASS means | FAIL means |
| --- | --- | --- |
| `pad_opens_two_recipients_slots` | The pad `B` uses for `M` is bit-identical to the pad `B` uses for `A`, **and** to the pad `A` uses for `B`. One plaintext complaint response therefore opens two other slots. F-VAL-001's algebraic core is `E1`-confirmed. | Some binding of `q` to its publisher, or a direction/identity tweak in the pad, exists that the audit missed. F-VAL-001 and F-VAL-002 both collapse; report the counter-evidence. |
| `impostor_share_verifies_and_group_finalizes` | Two things: (a) _without_ the harvest, all six peers reject `M`'s share — the control, showing the attack is not vacuous; (b) _with_ the harvest, every peer's `verify_encrypted_secret_share` returns `Ok`, `finalize` succeeds for all seven, and the group has one key. `M` is never complained about. This is Critic C-VAL-A's step 5, the link most likely to be wrong. | If the panic fires with `peer == IMPOSTOR`, step 5 is refuted: the attacker _is_ detectable, the group would be marked `COMPROMISED`, and F-VAL-001 drops to a griefing/abort finding (High at most). If it fires with `naive_rejections != 6`, the control assumption is wrong and the whole model needs re-deriving. |
| `impostor_recovers_victim_signing_share` | The final `assert_eq!` compares the reconstructed scalar with `key_shares[&VICTIM].as_key_package.signing_share`. Equal ⇒ `M` holds `A`'s **complete** FROST signing share. F-VAL-001 is reproduced end to end; certainty ≥ 90, severity Critical confirmed. | If only this one fails, the leak is _partial_: `M` holds `s_A` minus some term. Still a share-material leak, but the "complete signing share" wording, and therefore the `n = 7, m = 2` forgery arithmetic, must be corrected. Print `sum` and `real` and diff. |

Runtime: seconds. No network, no Anvil, no database.

## 5. Known mechanical gap

`identifier_scalar` converts a `frost_secp256k1::Identifier` to a `k256::Scalar` via `Identifier::serialize`. The return type of `serialize` differs across `frost-core` 3.x point releases (`Vec<u8>` vs. the ciphersuite `Serialization` type); the helper is written against `AsRef<[u8]>`, which both satisfy, but this could not be checked — `frost-core 3.0.0` is not on disk (A6). If it does not compile, the one-line replacement is `frost_core::Identifier::to_scalar(&id)`; the validator already enables `frost-core`'s `internals` feature (`crates/validator/Cargo.toml:11`). See `../../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` VAL-Q1.

Everything else — `keygen::setup`, `Secrets::commitment`, `verify_commitment`, `generate_secret_shares`, `verify_secret_share`, `verify_encrypted_secret_share`, `reveal_secret_share`, `finalize`, `group_commitments`, `KeyShare::as_key_package`, `GroupCommitments::group_key`, `marshal::frost_scalar`, `marshal::solidity_scalar`, `participants::identifier`, `bindings::{Point, KeyGenCommitment, KeyGenSecretShare}` — was read at its definition and matched against its use in `crates/validator/src/frost/mod.rs`'s existing `ceremony` test, which this PoC is deliberately shaped to mirror.

## 6. Not covered here

The state-machine and onchain halves of the Trigger are **not** exercised by this PoC:

- that `handle_key_gen_complained` really queues a `KeyGenComplaintResponse` for a plaintiff that has not itself shared (`state/keygen.rs:660-757`) — a `core::state` unit test could assert this without Anvil, and is the natural companion; it is also the regression test for remediation option 4;
- that `FROSTCoordinator.keyGenCommit` accepts a duplicate `q` and `keyGenComplain` is callable in `SHARING` — Solidity, needs `forge`;
- that the group reaches `FINALIZED` onchain with `M` inside.

Those are the phase-7B Anvil flow test the finding names. The cryptographic core above is the part that decides whether the finding is real, and it needs neither.

## 7. Fix-verification

After remediation option 1 (KDF-bound pad), `pad_opens_two_recipients_slots` **must fail** and `impostor_share_verifies_and_group_finalizes` **must fail at the impostor's `verify_encrypted_secret_share`**. Keep both as inverted regression tests rather than deleting them.
