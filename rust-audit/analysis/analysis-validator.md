> Generated at commit 82b3e0d by a read-only analysis agent (Claude Fable 5.1) with no toolchain available. Every statement is class I or E2 by inspection; nothing here was executed. The Manager spot-checked the citations of the top hypotheses only. Treat every hypothesis as a lead to confirm or refute, never as a finding.

# Safenet `validator` crate — technical map and risk analysis

Scope: `crates/validator` at commit `82b3e0d` (main). Every `.rs` file in the crate was read in full (8,098 LOC incl. tests). Supporting reads: `crates/core/src/{driver,effects,kdf,lib}.rs`, `crates/core/src/state/mod.rs`, `crates/core/src/tx/signer.rs`, targeted greps of `crates/core/src/index/{events,blocks,mod}.rs` and `utils.rs`; Solidity `FROSTCoordinator.sol`, `Consensus.sol`, `libraries/{FROST,FROSTSignatureShares,FROSTNonceCommitmentSet,FROSTGroupId,FROSTParticipantMap,ConsensusMessages,EpochRollover,Secp256k1,SafeTransaction,FROSTSignatureId}.sol`; docs `overview.md`, `validator-handbook.md`, the flow-test epic; commits `f01a3ea` (#803), `b7f646b` (#851), `40467c5`/`33fcdcc` (#834).

Not read: `frost-core`/`frost-secp256k1` 3.0.0 sources (no local registry; `cargo` absent) — every claim about upstream behaviour is marked _(upstream, not read)_. Also not read: `core/src/tx/mod.rs` (queue submission policy), `core/src/state/storage.rs` (snapshot table layout), `core/src/index/blocks.rs` beyond the #834 diff and greps.

Conventions: `file:line` refers to `crates/validator/src/` unless prefixed with `core/` (= `crates/core/src/`) or `sol:` (= `contracts/src/`). "E2" = defect visible in cited code with a concrete input; "I" = inference without a concrete trigger.

---

## 1. Purpose and runtime shape

**Binary wiring (`main.rs`).** `main.rs:40-46` loads the TOML config, initialises observability + metrics, connects the RPC `Provider` and a single SQLite pool. `main.rs:48-52` resolves the `FROSTCoordinator` address by calling `Consensus.getCoordinator()` on the configured `consensus` address (the coordinator is _not_ configured; it is trusted transitively via `consensus`). `main.rs:56-57` builds the watched address set `[consensus, coordinator] ++ config.validator.oracles`. `main.rs:62-79` constructs `ValidatorService` and the core `Driver` (indexer + state machine + effect manager + durable tx queue, all over the same pool). `main.rs:81-93` reconciles the onchain staker association with a one-off `Action::SetValidatorStaker` queued outside the state machine. `main.rs:96` runs the driver until shutdown.

**Service trait impl (`service/mod.rs`).** `ValidatorService` (`service/mod.rs:26-39`) holds the account, the `SecretStore`, the genesis `ParticipantSet` (derived purely from config: `service/mod.rs:51-57`), the EIP-712 `ConsensusDomain(chain_id, consensus)` (`service/mod.rs:58`), the coordinator address and the config. `Service::components` (`service/mod.rs:93-116`) splits it into the pure `state::Transition`, the impure `effect::Handler` and the `action::Encoder`. The event set is the `watcher_events!` enum `Event { Consensus, Coordinator, Oracle }` (`service/mod.rs:71-80`); the macro (`core/index/events.rs:557-593`) decodes a raw log by trying each contract's `decode_raw_log` in order and never consults the emitting address.

**Event flow.** Driver loop (`core/driver.rs:170-198`) selects between the next indexer update and the next completed effect (`core/driver.rs:206-231`). For an `Update`, the tx queue is told the block status, then `StateMachine::handle_update` applies `Message::NewBlock` (one transition) or `Message::Event` (one transition per log, then a snapshot commit at the range's last block: `core/state/mod.rs:190-239`). For a `Resume`, `handle_resume` mutates live state _without_ committing (`core/state/mod.rs:250-258`). Returned commands are split: `Command::Action` → `Encoder::encode_action` → `(Transaction, expires_at)` pushed to the durable queue; `Command::Effect` → spawned as a concurrent tokio task (`core/driver.rs:266-284`, `core/effects.rs:53-62`). Effects therefore (a) run concurrently and complete in arbitrary order (`core/state/mod.rs:45-51`), (b) may be replayed after a crash/reorg (`core/state/mod.rs:60-73`, `core/effects.rs:20-24`), and (c) are _not durable_: a resume that lands after snapshot N is only persisted with the commit of N+1's logs.

**Dispatch (`state/mod.rs:402-501`).** `apply_transition` routes 14 events to handlers (`state/mod.rs:415-463`), runs five per-block routines in a fixed order on `NewBlock` (`state/mod.rs:464-481`: rollover clock → keygen timeouts → signing timeouts → nonce top-up → group reconciliation; note the reconciliation effect is emitted _last_), and handles four resumes (`state/mod.rs:482-498`).

**State machine phases and timeouts (all block-based, `config.rs:88-104` defaults):**

| Phase | State | Enter | Deadline / timeout | Timeout handler |
| --- | --- | --- | --- | --- |
| Genesis wait | `RolloverState::WaitingForGenesis` (`state/mod.rs:93`) | default | none | — |
| DKG round 1 | `CollectingCommitments` (`state/mod.rs:113-126`) | `start_key_gen` (`state/keygen.rs:1115-1180`), emits `Effect::KeyGenSetup` | `deadline` = start block + `key_gen_timeout` (120); genesis: `None` | `handle_key_gen_timeouts` `state/keygen.rs:1027-1046` (exclude non-committers); "stuck setup" `1003-1024` (skip epoch) |
| DKG round 2 | `CollectingShares` (`state/mod.rs:129-146`) | last valid commitment (`state/keygen.rs:247-261`) or late setup resume (`132-140`) | last-commitment block + 120 | `1047-1069` (exclude non-sharers) |
| DKG round 3 | `CollectingConfirmations` (`state/mod.rs:149-165`) | last share (`state/keygen.rs:371-379`) | `complain` = +120, `response` = +240, `confirm` = +360 from last-share block | `1070-1095` |
| Rollover signing | `SigningRollover` (`state/mod.rs:168-177`) + a `SigningState` keyed by the rollover message | final confirmation (`state/keygen.rs:508-562`) | signing session deadline `signing_timeout` (6) | `handle_signing_timeouts` |
| Staged / skipped / halted | `EpochStaged`, `EpochSkipped`, `Halted` | `handle_epoch_staged` (`state/keygen.rs:580-647`), `rollover_failure` (`1426-1441`) | block clock `handle_rollover_new_block` (`912-981`) | — |
| Preprocess | `NonceState` per epoch (`state/mod.rs:68-76`) | `finalize_key_gen` reserves chunk 0 and emits `Effect::NonceTree` (`state/keygen.rs:1295-1326`); top-up below 100 remaining (`state/preprocess.rs:17,85-103`) | none (Preprocess action never expires: `service/action.rs:250-254`) | — |
| Sign: request | `SigningState::WaitingForRequest` (`state/mod.rs:281-300`) | `TransactionProposed` (`state/transactions.rs:16-76`) / final confirmation (`state/keygen.rs:529-548`) / restart | `signing_timeout` (6) | `state/sign.rs:578-616` |
| Sign: oracle | `WaitingForOracle` (`state/mod.rs:303-320`) | `Sign` event for a transaction packet (`state/sign.rs:46-71`) | `oracle_timeout` (12) | `617-630` (drop) |
| Sign: nonces | `CollectNonceCommitments` (`state/mod.rs:323-342`) | `Sign` (rollover) / approved `OracleResult` | 6 | `631-661` (retain revealers, restart) |
| Sign: shares | `CollectSigningShares` (`state/mod.rs:346-363`) | all signers revealed (`state/sign.rs:322-344`), emits `Effect::UseNonce` | 6 | `662-701` (canonical selection ≥ threshold, restart) |
| Sign: attest | `WaitingForAttestation` (`state/mod.rs:366-373`) | `SignCompleted` (`state/sign.rs:442-475`) | 6 | `702-720` (direct `stageEpoch`/`attestTransaction` fallback) |

---

## 2. Module map

| File | LOC | Responsibility | Key pub types / fns | Main deps |
| --- | --- | --- | --- | --- |
| `main.rs` | 99 | CLI, wiring, staker reconcile | `Options`, `main` | argh, safenet-core `Driver`, alloy |
| `config.rs` | 290 | TOML schema + defaults | `Config`, `ValidatorConfig`, `Participant` | serde, toml, sqlx options, core `driver::Config` |
| `bindings.rs` | 247 | `sol!` ABI: structs, `Consensus`, `Coordinator`, `Oracle` | `Point`, `Signature`, `SafeTransaction`, `KeyGenCommitment`, `KeyGenSecretShare`, `SignNonces`, `SignSelection`, `SignatureShare`, `Callback` | alloy `sol!` |
| `metrics.rs` | 132 | Prometheus counters | `transitions_total`, `effects_total`, `initialize` | metrics |
| `merkle.rs` | 142 | Sorted-pair keccak Merkle tree | `MerkleTree::{build,root,proof}`, `MerkleRoot` | alloy keccak |
| `service/mod.rs` | 129 | `Service` impl, event enum | `ValidatorService`, `Event` | core `driver::Service`, `watcher_events!` |
| `service/action.rs` | 381 | `Action` enum → calldata + fixed gas + expiry | `Action`, `Encoder` | alloy `SolCall`, core `ActionEncoder` |
| `service/effect.rs` | 275 | Impure effects (RNG, secret store, nonce generator) | `Effect`, `Resume`, `Handler` | rand `thread_rng`, tokio `Mutex`, core `EffectHandler` |
| `state/mod.rs` | 515 | Snapshotted `State`, all state enums, dispatch | `State`, `RolloverState`, `SigningState`, `Packet`, `Transition` | serde, core `StateTransition` |
| `state/keygen.rs` | 1459 | DKG/epoch rollover handlers, timeouts, restarts | `handle_*`, `start_key_gen`, `restart_key_gen_excluding`, `fail_rollover!` | frost::keygen, consensus::group |
| `state/sign.rs` | 868 | Signing ceremony handlers, timeouts, callbacks | `handle_sign*`, `handle_nonces`, `handle_signing_timeouts`, `Packet::{attestation_callback,attestation_action}` | frost::sign, hashing |
| `state/preprocess.rs` | 248 | Nonce chunk bookkeeping | `handle_nonce_tree`, `handle_preprocess`, `handle_nonce_topup`, `handle_group_reconciliation`, `NonceState` | frost::preprocess |
| `state/transactions.rs` | 101 | Transaction proposal / attestation | `handle_transaction_proposed`, `handle_transaction_attested` | hashing |
| `consensus/epoch.rs` | 95 | Epoch ids and block→epoch | `EpochId`, `next_number` | — |
| `consensus/group.rs` | 459 | Participant sets, group ids, thresholds | `Group`, `ParticipantSet`, `participants_set`, `Epoch` | merkle, keccak |
| `consensus/hashing.rs` | 249 | EIP-712 message hashes | `ConsensusDomain`, `safe_tx_hash`, `safe_tx_struct_hash`, `transaction_proposal_hash`, `epoch_rollover_hash` | alloy `sol!`/`Eip712Domain` |
| `frost/mod.rs` | 258 | Module root + end-to-end ceremony test | — | frost-secp256k1 aggregate (test) |
| `frost/keygen.rs` | 516 | DKG parts 1–3 over address identifiers with ECDH-encrypted shares | `Secrets`, `setup`, `verify_commitment`, `GroupCommitments`, `SharingState`, `generate_secret_shares`, `verify_secret_share`, `verify_encrypted_secret_share`, `verify_revealed_secret_share`, `reveal_secret_share`, `KeyShare`, `finalize` | frost-core `dkg::{part1,part2,part3}`, `verify_proof_of_knowledge`, `sum_commitments` |
| `frost/sign.rs` | 204 | Signature share + signer selection root | `RevealedNonces`, `verify_revealed_nonces`, `SignatureShare`, `signature_share`, `signer_leaf` | frost-core `round2::sign`, `compute_binding_factor_list`, `compute_group_commitment`, `derive_interpolating_value` (`internals`) |
| `frost/preprocess.rs` | 189 | 1024-nonce chunk generation + Merkle commitment | `SEQUENCE_CHUNK_SIZE`, `decode_sequence`, `Nonces`, `NonceChunk::{generate,with_size}`, `nonces_leaf` | rand_chacha `ChaCha12Rng`, rayon, frost `round1::SigningNonces` |
| `frost/ecdh.rs` | 181 | ECDH-XOR share encryption | `EncryptionKey`, `EncryptionPublicKey`, `ecdh` | k256, hash2curve, zeroize |
| `frost/marshal.rs` | 176 | ABI ⇄ k256/frost conversions | `solidity_*`, `frost_*` | k256 sec1, frost types |
| `frost/participants.rs` | 33 | Address → FROST identifier | `identifier` | frost `Identifier::derive` |
| `frost/error.rs` | 46 | Error taxonomy | `Error::{Unexpected,Participant}`, `Culprit` | thiserror |
| `secrets/mod.rs` | 6 | re-exports | `SecretStore` | — |
| `secrets/store.rs` | 447 | Reorg-immune SQLite secret store | `SecretStore::{new,store_keygen_secrets,retain_keygen_secrets,register_nonces_chunk,nonces_reveal,take_nonce,retain_nonces}` | sqlx, serde_json |
| `secrets/nonces.rs` | 348 | Per-group background nonce streams | `NonceGenerator::{start,next,retain}`, `NonceStream` | std thread/mpsc, tokio `Semaphore`/`oneshot` |

Workspace versions (Cargo.lock): frost-core 3.0.0, frost-secp256k1 3.0.0, k256 0.13.4, rand 0.8 (`Cargo.toml:13`), rand_chacha 0.3 (`Cargo.toml:14`), rayon 1, alloy 2.

---

## 3. External interfaces

**Contract calls (bindings.rs → action.rs encoder).** Coordinator: `keyGenAndCommit` (gas 250k, `action.rs:123-148`), `keyGenSecretShare` (250k + 25k·|f|, `149-169`), `keyGenComplain` (300k, `170-187`), `keyGenConfirm`/`keyGenConfirmWithCallback` (200k/300k, `188-216`), `keyGenComplaintResponse` (300k, `217-236`), `preprocess` (250k, **no expiry**, `237-255`), `signRevealNonces` (250k, `256-275`), `signShareWithCallback` (400k, `276-299`; the non-callback `signShare` binding exists but is never emitted), `sign` (150k, `300-317`). Consensus: `attestTransaction` (250k, `318-345`), `stageEpoch` (250k, `346-367`), `setValidatorStaker` (100k, `368-378`), and view calls `getCoordinator`, `getValidatorStaker` (`main.rs:49-52,83-86`). Gas limits are constants (no estimation); an under-provisioned constant is a silent liveness failure (see checklist).

**Events consumed** (`bindings.rs:113-146,172-209,245`; dispatch `state/mod.rs:415-459`): Coordinator `KeyGen`, `KeyGenCommitted`, `KeyGenSecretShared`, `KeyGenConfirmed`, `KeyGenComplained`, `KeyGenComplaintResponded`, `Preprocess`, `Sign`, `SignRevealedNonces`, `SignShared`, `SignCompleted`; Consensus `EpochStaged`, `TransactionProposed`, `TransactionAttested`; Oracle `OracleResult`. `EpochProposed` and `ValidatorStakerSet` are decoded but ignored (`state/mod.rs:461-462`). Only `OracleResult` is bound to its emitting address (`state/mod.rs:459`, `state/sign.rs:189,226`); every other event is accepted from any watched address (see H2).

**Config (`config.rs`).** Top level: `rpc: Url`, `signer` (hex private key; `core/tx/signer.rs:84-94` zeroizes the raw bytes and `Debug` prints only the address `96-100`), `database` (SQLite connect options), `[validator]`, `[observability]`, flattened driver config (`[index]`, `[transactions]`). `[validator]` (`config.rs:53-86`): `consensus: Address` (required), `staker: Option<Address>`, `participants: Vec<Participant>` (default empty), `oracles: BTreeSet<Address>` (default empty ⇒ every proposal ignored: `state/transactions.rs:32-40`), `genesis_salt: B256` (default zero ⇒ genesis context = zero hash, `consensus/group.rs:288-297`), `blocks_per_epoch = 1440`, `key_gen_timeout = 120`, `signing_timeout = 6`, `oracle_timeout = 12` (all `NonZeroU64`; `config.rs:88-104`). `Participant { address, active_from = 0, active_before: Option<NonZeroU64> }` (`config.rs:108-120`). Validation: only serde `deny_unknown_fields`; the genesis set must be viable or `ValidatorService::new` fails (`service/mod.rs:51-57`). No validation that `signing_timeout` < `blocks_per_epoch`, no uniqueness check on participants (BTreeSet dedupes), no check that the local account is in the set (it may legitimately observe).

**SQLite tables owned by the validator (`secrets/store.rs:66-93`):** `keygen_secrets(group_id TEXT, address TEXT, secrets TEXT JSON, PK(group_id,address))`; `nonces_chunks(root TEXT PK, group_id, address)`; `nonces(root, offs INTEGER, nonce TEXT JSON, PK(root,offs), FK root ON DELETE CASCADE)`; index on `nonces_chunks(group_id)`. Shared pool also holds core's `snapshots` (state, incl. key shares and DKG state in JSON) and the tx queue tables (not read). All plaintext (handbook §Consensus Secrets).

**Metrics (`metrics.rs`).** `safenet_validator_transitions_total{kind=block|event|resume}` (`36-43`) and `safenet_validator_effects_total{effect,result}` (`111-120`), pre-materialised at zero (`123-132`). No metric exposes ceremony progress or nonce inventory.

---

## 4. Trust boundaries and untrusted inputs

All protocol inputs arrive as decoded logs from the configured RPC. The RPC is trusted for chain data (the indexer verifies hash chaining and optionally blooms, `core/index/events.rs:445-463`); a lying RPC can only cause liveness loss or force an exit on a >`max_reorg_depth` (default 5, `core/index/blocks.rs:83`) reorg (#834).

| Input (from other participants / proposers) | Parsed by | Validation before use | What a malicious party can inject |
| --- | --- | --- | --- |
| `KeyGenCommitted{participant, commitment{q,c[],r,mu}}` | `frost::keygen::verify_commitment` (`frost/keygen.rs:79-100`) via `marshal::frost_commitment` (`frost/marshal.rs:105-122`) | `q` non-identity (`ecdh.rs:70-75`); each `c[i]` decoded as a valid curve point (identity allowed); `(r,mu)` decoded; PoK verified with frost-core; **no membership check** (`state/keygen.rs:173-175`) and **no uniqueness / possession check on `q`** | A copied or related encryption key `q` (H1). An invalid PoK stalls the round until timeout (excluded then). |
| `KeyGenSecretShared{participant, share{y, f[]}}` | `verify_secret_share` (`frost/keygen.rs:267-299`), then own slot via `verify_encrypted_secret_share` (`337-390`) | participant ∈ commitments; `y == Φ(id)` from summed commitments; own ciphertext decrypted and checked `g^s == Φ_peer(me)` (`302-330`); `f` length trusted to the contract (`sol:FROSTCoordinator.sol:430`) | Invalid share ⇒ local complaint (`state/keygen.rs:342-346`). Wrong `y` ⇒ ignored ⇒ timeout exclusion. |
| `KeyGenComplained{plaintiff, accused}` | `handle_key_gen_complained` (`state/keygen.rs:654-757`) | none beyond phase/deadline guards; counts per accused; if `accused == self` reveals `f_me(plaintiff)` in plaintext (`734-745`) | Unlimited complaints by one plaintiff against distinct accused (contract limits one per pair, `sol:FROSTParticipantMap.sol:183`); each forces a plaintext reveal (H1). |
| `KeyGenComplaintResponded{plaintiff, accused, secretShare}` | `verify_revealed_secret_share` (`frost/keygen.rs:398-415`) | scalar canonical (`marshal.rs:125-129`), `g^s == Φ_accused(plaintiff)`; only counted if an unresponded complaint exists (`state/keygen.rs:821-826`) | An invalid reveal restarts the DKG excluding the accused (`842-859`). |
| `KeyGenConfirmed{participant}` | `state/keygen.rs:448` | none (set insert) | — |
| `Preprocess{participant, chunk, commitment}` | `state/preprocess.rs:55-81` | only own events used | — |
| `Sign{gid, message, sid, sequence}` (any EOA may call `sign`, `sol:FROSTCoordinator.sol:530-542`) | `handle_sign` (`state/sign.rs:22-133`) | matched against a locally-opened session by `message`; **the nonce sequence is advanced for every `Sign` of a tracked group before matching** (`30-34`) | Sequence burning / nonce exhaustion griefing (H6). |
| `SignRevealedNonces{sid, participant, nonces{d,e}}` | `verify_revealed_nonces` (`frost/sign.rs:37-43`, `marshal.rs:163-176`) | participant ∈ `signers`; points valid & non-identity; Merkle inclusion trusted to the contract | Re-reveal to become `last_signer` (contract does not dedupe reveals) – delays restarts (H8). |
| `SignShared{sid, selectionRoot, participant, z}` | `state/sign.rs:410-435` | none (bookkeeping only; comment explains reliance on onchain proof) | A share to a foreign root is irrelevant. |
| `SignCompleted`, `EpochStaged`, `TransactionAttested` | `state/sign.rs:442-508`, `state/keygen.rs:580-647`, `state/transactions.rs:80-100` | matched by sid/message/gid; signature _not_ re-verified locally (contract verified) | — |
| `TransactionProposed{oracle, epoch, oracleData, transaction}` (any EOA may propose) | `state/transactions.rs:16-76` | epoch ∈ participating epochs; `oracle ∈ config.oracles`; dedupe by message | Spam proposals ⇒ sessions + `WaitingForOracle` (12-block) per message; each consumes one sequence number for every validator. |
| `OracleResult{requestId, approved}` | `state/sign.rs:172-249` | `log.address == expected oracle` | A malicious allow-listed oracle approves anything (design). |
| Any Coordinator/Consensus-shaped event from an allow-listed oracle address | `watcher_events!` decode ignores address | **none** | Full protocol-event injection (H2). |

Malicious participant (< 1/3): can withhold at any round (excluded by timeouts), submit invalid commitments/shares (excluded or complained), file bogus complaints (each forces one plaintext reveal of _that plaintiff's_ share), re-reveal nonces, refuse to restart when responsible (costs one extra `signing_timeout`), and — via H1 — learn an honest participant's key share while remaining a member. Malicious proposer: only liveness/griefing (proposals need an allow-listed oracle's approval to progress; `Coordinator.sign` is permissionless).

---

## 5. Invariants

| Invariant | Enforced where | Status |
| --- | --- | --- |
| A nonce pair is used for at most one signing package | `take_nonce` deletes the row atomically (`secrets/store.rs:205-218`); `Resume::Nonce` applied once (`state/sign.rs:359-404`); replay ⇒ `None` ⇒ `Resume::Noop` (`service/effect.rs:189-201`); own-commitment consistency check inside frost-core `round2::sign` _(upstream, not read)_ | Enforced (modulo DB restore from backup, §6) |
| Nonce deleted from memory and storage before the share is broadcast | Delete-then-compute: `take_nonce` → resume → `signature_share` consumes `Nonces` by value (`state/sign.rs:377`) → action queued. `Nonces` drop relies on frost-core zeroize _(upstream, not read)_; the JSON `String` read from SQLite is not zeroized | Enforced for storage; memory hygiene assumed |
| Key shares / coefficients never in logs | Custom redacting `Debug` for `Nonces` (`frost/preprocess.rs:64-71`), `NonceChunk` (`150-159`), `EncryptionKey` (`frost/ecdh.rs:50-54`); `Secrets`, `SharingState`, `KeyShare`, `VerifiedShare` _derive_ `Debug` (`frost/keygen.rs:27,144,302,433`) and are printed at `warn` on effect failure (`service/effect.rs:249`, `?effect` includes `KeyShare`) and at `trace` for resumes (`core/effects.rs:59`, `core/driver.rs:261`) — safe only if frost-core redacts `SigningShare`/`SecretPackage` | Assumed, relies on upstream |
| Identifiers unique and bound to addresses | `Identifier::derive(address)` (`frost/participants.rs:16-19`) = HID over the 20-byte address; matches `sol:FROST.sol:76-78,321-326`; commitments keyed by address in `BTreeMap`s | Enforced (collision negligible) |
| Selection root deterministic across honest nodes | Leaves ordered by identifier (`frost/sign.rs:91-109`), built from the full `revealed` map which equals `signers` when `UseNonce` fires (`state/sign.rs:304-320`); `hash_pair` commutative (`merkle.rs:87-93`) | Enforced given identical event views |
| Group tainted after too many complaints against one participant | `complaint.total >= threshold` ⇒ restart excluding accused (`state/keygen.rs:721-731`); contract marks `COMPROMISED` at the same bound (`sol:FROSTCoordinator.sol:480-484`) | Enforced per accused only — complaints spread across many accused are unbounded (H1) |
| Rollover never enters a dishonest-majority set | `participants_set` requires `count >= 2·total/3 + 1` (`consensus/group.rs:206-216,227-240`); a validator only joins/attests groups computed from _its own_ config (`state/keygen.rs:1115-1180`, `41-54`, `521-526`) | Enforced locally; assumes honest operators share the same `participants` config (not onchain) |
| Sequence numbers are consumed monotonically | `NonceState::observe` (`state/preprocess.rs:180-194`) | Enforced (also the griefing vector, H6) |
| Own commitment never republished after reorg replay | `commitments.contains_key(&self.account)` (`state/keygen.rs:94-96`); secrets reused by `ON CONFLICT` no-overwrite (`secrets/store.rs:105-124`) | Enforced |
| Genesis group id is externally authorised and never restarted | `restart_key_gen_excluding` halts for genesis (`state/keygen.rs:1204-1210`) | Enforced (but see H4) |
| Threshold = n/2+1; contract requires threshold > 1 | `group_threshold` (`consensus/group.rs:220-222`), `min_participants` floor 2 | Enforced |

---

## 6. Persistence, restarts and reorgs

**Two stores, one file.** (1) Core's snapshot store: `State` serialised per committed block, rolled back by `snapshots.reorg(number)` on `BlockUpdate::Uncle` (`core/state/mod.rs:182-189`), pruned below the safe block. It contains `Epoch.key_share`, `KeyGenCommitment::Participating{secrets}`, `SharingState` (incl. `peer_packages` = the plaintext shares this validator sent to every peer, `frost/keygen.rs:144-150`), and `NonceState` chunk→root links. (2) The validator's `SecretStore` (`secrets/store.rs:1-22` module doc), deliberately never rolled back: `keygen_secrets` keyed by `(group_id, address)`, insert-only (`105-124`); nonce chunks keyed by Merkle root, nonces by `(root, offs)`; `nonces_reveal` is non-consuming (`177-196`), `take_nonce` is `DELETE … RETURNING` (`205-218`); both tables are reconciled every block by `retain_*` against the groups the state machine still tracks (`service/effect.rs:202-238`).

**Reorg of a consumed nonce (overview "Nonces and Reorgs").** Sequence `s` bound to `m` at block `b`; reveals; `UseNonce` deletes `(root,offset)`; share queued. Uncle `b` ⇒ snapshot rolls back to `WaitingForRequest{m}`. New branch `Sign(s, m')` ⇒ `observe(s)` returns the same `(root,offset)` (`state/preprocess.rs:180-194`) ⇒ `Effect::RevealNonceCommitments` ⇒ `nonces_reveal` returns `None` ⇒ `Resume::Noop` ⇒ this validator never reveals for `m'` and is excluded on timeout. The already-queued share for `m` (still in the durable queue, expiring at `deadline`) reverts onchain under `m'` (challenge mismatch) but publishes `z` for the _original_ package — a single use. Same-branch replay of `m`: `take_nonce` ⇒ `None` ⇒ no second share. Property holds as long as the store is never restored from a backup taken before the deletion; the handbook recommends backups (`docs/validator-handbook.md:19,75`) without stating that a restore after a reorg can resurrect burned nonces.

**Reorg of DKG setup.** `KeyGenSetup` re-run after rollback reuses stored secrets (`store_keygen_secrets` returns the existing row); if the own commitment was replayed before the resume, no second publish (`state/keygen.rs:94-107`); if all commitments already landed, the round finalises from the resume (`132-145`). Mismatch between stored secrets and the onchain commitment (e.g. DB restored) fails safe via `IncorrectCommitment` ⇒ `rollover_failure` (`frost/keygen.rs:181-183`, `state/keygen.rs:1270-1275`), which for genesis is `Halted`.

**Restart mid-ceremony.** The state machine resumes from the last committed snapshot and the indexer replays from there (`core/driver.rs:129-132`, `core/state/mod.rs:135-143`). Effects in flight at shutdown are aborted with the `EffectManager` and never re-spawned; resumes applied after the last commit are lost. Consequences per effect: `KeyGenSetup` lost ⇒ `secrets: None` forever — non-genesis is skipped by the #851 "stuck" branch (`state/keygen.rs:1003-1024`, requires `deadline: Some`), genesis has no deadline (H4); `NonceTree` lost ⇒ the `None` chunk reservation persists and counts as capacity (H3); `RevealNonceCommitments` lost ⇒ miss one ceremony; `UseNonce` lost after deletion ⇒ nonce burned without a share (safe); `StartNonceGeneration`/`ReconcileGroupSecrets` are re-issued every block. The process-local `NonceGenerator` is empty after restart and only repopulated by the next block's reconcile effect, which is spawned _after_ the same block's `NonceTree`/top-up effects (`state/mod.rs:470-479`) (H3).

**Fix commits.** #803 (`f01a3ea`): `retain_nonces` now keeps nonces for groups with _and_ without a key share because a reorg/restart can roll a key share back to `None` after nonces were generated (`service/effect.rs:219-228`). #851 (`b7f646b`): merged `WaitingForSetup` into `CollectingCommitments` so peer commitments are collected while the setup effect is outstanding, and added the "stuck setup ⇒ skip epoch" timeout; the genesis gap noted in H4 remains. #834 (`40467c5`): the block watcher keeps an explicit safe anchor and returns `ExceededMaxReorgDepth`, which the driver turns into a process exit (`core/driver.rs:206-231`); the commit message notes the safe block hash is not persisted, so a reorg coinciding with a restart is not detected.

---

## 7. Concurrency and cancellation

- **Rayon is not used inside async code.** `NonceChunk::with_size` uses `into_par_iter` (`frost/preprocess.rs:123-131`) but is only called from the dedicated `std::thread` worker (`secrets/nonces.rs:109-114,123-143`). Each stream eagerly computes one chunk ahead and blocks on `mpsc::recv` (`145-192`). Worker threads are detached; dropping the stream closes the channel and the thread exits after its current chunk.
- **Handler sharing.** `effect::Handler` is behind an `Arc` and effects run as concurrent tokio tasks (`core/effects.rs:53-62`). The only intra-handler lock is `tokio::Mutex<NonceGenerator>`, released before awaiting a chunk (`service/effect.rs:155-158`, `secrets/nonces.rs:203-209`). One in-flight request per group via a `Semaphore(1)`; duplicates resolve to `Ok(None)` ⇒ `Resume::Noop` (`service/effect.rs:159-162`).
- **CPU on the driver task.** `generate_secret_shares`, `finalize`, `signature_share`, `verify_*` all run synchronously inside `apply_transition`; sizes are tiny (n ≤ tens), acceptable. `dkg::part1` runs inline in the effect task (`service/effect.rs:131-134`).
- **Ordering assumptions.** Core states resume order is undefined (`core/state/mod.rs:45-51`). The validator guards: `Resume::Setup` by group id (`state/keygen.rs:82`), `Resume::NonceCommitments` by sid (`state/sign.rs:146-153`), `Resume::Nonce` by message + phase (`state/sign.rs:365-375`) and, for a stale nonce vs. a restarted session, on frost-core's own-commitment check _(upstream, not read)_. `ReconcileGroupSecrets` is computed from the state at the _start_ of a block's `NewBlock` transition but executes concurrently with effects spawned by that block's logs (H7). SQLite pool: default `max_connections` (sqlx default 10), recycling disabled (`core/utils.rs:56-62`) — DB operations from different effects genuinely interleave.
- **Timeouts** are all derived from event block numbers, so they are deterministic across honest nodes; one exception is the late-setup finalisation deadline `old_deadline + key_gen_timeout` (`state/keygen.rs:132-133`) which differs from peers' `last_commitment_block + timeout` (`251-252`) — local, transient divergence only.
- **Cancellation.** Ctrl-C breaks the driver loop after the in-progress update completes (`core/driver.rs:175-197`); the transaction queue is durable, effects are not (see §6).

---

## 8. Error handling — panics, casts, indexing in non-test code

Mechanical sweep of every `unwrap/expect/panic!/unreachable!/as/[i]` outside `#[cfg(test)]`:

| Site | Construct | Provably safe? |
| --- | --- | --- |
| `config.rs:90,94,98,102` | `NonZeroU64::new(const).unwrap()` in `const fn` | Yes (non-zero literals) |
| `consensus/group.rs:143-146` | `len().try_into().expect(...)` to `u16` | Yes — `participants_set` already bounds `addresses.len()` via `u16::try_from` (`206,211`) |
| `consensus/group.rs:231,237` | `count as u32`, `min as u16` | Yes — `min ≤ max(count,1) ≤ 65535`; `debug_assert` only |
| `consensus/group.rs:256-262,273-275,294-295` | fixed slice writes | Yes (constant sizes) |
| `consensus/hashing.rs:62` | `tx.operation as u8` | Yes (fieldless `sol!` enum) |
| `frost/ecdh.rs:130-131` | `expect` on `hash_to_field`, `NonZeroScalar::new` | Yes / negligible (zero output) |
| `frost/ecdh.rs:115` | `.to_affine().x()` | Yes — pubkey non-identity, scalar non-zero, prime-order curve |
| `frost/marshal.rs:78-81` | `as_chunks::<32>()` + `chunks[0]`,`[1]` | Yes — uncompressed SEC1 is 65 bytes |
| `frost/marshal.rs:141-143` | fixed slices | Yes |
| `frost/participants.rs:18` | `Identifier::derive(..).expect` | Fails only if HID(address) ≡ 0 mod n — negligible |
| `frost/preprocess.rs:117` | `offset.checked_add(1).expect("chunk too large")` | Yes (offset < 1024) |
| `frost/preprocess.rs:28-32` | `/`,`%` by constant | Yes |
| `frost/sign.rs:156,159` | `expect` on binding-factor round trip | Yes (32-byte canonical scalar) |
| `frost/sign.rs:135` | `signers.range(..id).count()` as proof index | Yes (own id present, checked at `129-132`) |
| `merkle.rs:21-22,55` | `.get().unwrap_or(ZERO)` | Yes; `proof(index ≥ len)` returns a wrong-but-non-panicking proof |
| `secrets/store.rs:159,187,211` | `i64::try_from(offset)?` | Handled |
| `service/action.rs:154` | `25_000 * share.f.len() as u64` | Yes ( | f | ≤ 65534) |
| `state/keygen.rs:109,196,351,385,449,879,1010` | `map.len() as u16` compared to `count` | Truncation only if > 65535 entries; bounded by contract membership. With injected events (H2) a map can exceed `count` ⇒ round never closes (liveness), no panic |
| `state/keygen.rs:714-716` | `complaint.total += 1; unresponded += 1` (u16) | Overflow needs > 65535 complaints against one accused; contract allows one per plaintiff ⇒ ≤ count. `unresponded -= 1` guarded at `823` |
| `state/keygen.rs:1310` | `debug_assert_eq!(chunk, Some(0))` | Yes (fresh `NonceState`) |
| `state/sign.rs:538,596,680` | `threshold as usize` | Widening |
| `state/preprocess.rs:189,229,241,246` | `saturating_add`, `checked_add`, `saturating_sub`, `sum::<u64>` | Yes (sum needs 2^54 chunks) |
| `state/keygen.rs:474,518-520,1374-1378`, `consensus/epoch.rs:6-9` | `saturating_mul/add` for rollover block / epoch | Yes |

No `panic!`, `unreachable!`, `unimplemented!` or `todo!` in non-test code. All state transitions are total (`core/state/mod.rs:76-78` contract), failures are logged and mapped to `Resume::Noop` (`service/effect.rs:243-256`) or `rollover_failure`.

---

## 9. Cryptography and secrets

**RNG sources.** DKG setup: `rand::thread_rng()` (ChaCha12 reseeded from OS entropy) in the effect task (`service/effect.rs:132`), feeding `EncryptionKey::generate` (32 random bytes → `hash_to_scalar("enc")`, `frost/ecdh.rs:29-36,123-132`) and `dkg::part1` (`frost/keygen.rs:49-64`). Nonces: each worker thread owns a `thread_rng()` (`secrets/nonces.rs:124`); per chunk a `ChaCha12Rng` is seeded from it (`frost/preprocess.rs:114`) and cloned with `set_stream(offset+1)` per nonce pair (`115-118`), so the 1024 nonce pairs of a chunk come from distinct streams of one 256-bit seed; `round1::SigningNonces::new(signing_share, rng)` applies RFC 9591 `nonce_generate = H3(random || secret)` _(upstream, not read)_, so even a weak stream does not expose the nonce without the share. No `OsRng` direct use; no seed persistence.

**KDF.** `safenet_core::kdf::derive_key` (HKDF-SHA256, `core/kdf.rs:19-27`) is **not used by the validator** (grep: zero hits in `crates/validator/src`); only `Signer::derive_key` (`core/tx/signer.rs:59-64`, used by the sentinel) calls it. All validator secrets are sampled randomly and persisted, which is why the reorg-immune store exists. There is no deterministic derivation of coefficients or nonces from the signer key.

**DKG vs RFC 9591 / frost-core parts 1–3.** Part 1: `setup` = `dkg::part1(identifier, count, threshold)` + PoK (`frost/keygen.rs:53-57`); the published commitment is `(q, c[0..t), r, mu)` (`marshal.rs:25-38`). Verification of every commitment, including one's own: point decoding + `verify_proof_of_knowledge` (`frost/keygen.rs:86-93`); the contract does **not** verify the PoK (`sol:FROSTCoordinator.sol:365-383` only checks `q ≠ 0` and `|c| == threshold`), so PoK enforcement is purely offchain and a group whose PoK fails simply never gets honest confirmations. Part 2: `dkg::part2` over peer packages (`177`), an explicit self-commitment check (`181-183`), verifying share from summed commitments (`187-192`), per-peer ECDH encryption in ascending-address order excluding self (`196-214`). Share verification on receipt: `y == Φ(id)` (`277-289`) and own-slot decrypt + `SecretShare::verify` (`316-324`). Complaints: reveal `f_me(plaintiff)` plaintext (`420-428`); revealed shares verified against the accused's commitment (`398-415`). Part 3: `finalize` = self-package check (`478-483`) + `dkg::part3` (`489-494`). Deviation from RFC/frost-core: identifiers are `HID(address)`; the encryption key `q` is separate from `C[0]` and has **no proof of possession** (contrast `docs/overview.md` §KeyGen, which describes reusing `C[0]`, whose PoK would have covered it). Identity coefficient commitments are accepted by `frost_point` (`marshal.rs:136-138`); an all-honest group is unaffected (a zero constant term contributes nothing).

**ECDH share encryption (`frost/ecdh.rs`).** `pad = (Q_peer · sk_me).x` — the raw affine x-coordinate, unhashed — XORed byte-wise with the 32-byte big-endian share (`110-121`). Properties: (i) symmetric: `pad(A→B) == pad(B→A)`, so **each pairwise pad encrypts two values** (`f_A(B)` and `f_B(A)`), contradicting the one-value OTP argument in `overview.md` (test `ecdh_is_commutative` at `154-163` demonstrates it); (ii) no identity/direction binding and no PoP on `q`, so a participant who publishes another's `q` shares that participant's pads with every peer (H1); (iii) the x-coordinate is not uniform (only ~half of field elements are valid abscissae), leaking ≈1 bit per ciphertext — negligible but non-standard; (iv) the pad is reused deterministically across reorg replays (same secrets, same ciphertexts) — safe because the plaintext is identical. `EncryptionKey` is zeroized on drop (`56-62`) and `Debug`-redacted (`50-54`); public keys serialise compressed (`97-104`) and reject identity on deserialise (`83-95`). If `C[0]` had been reused as in the docs, a PoK-covered key would have prevented copying; what actually leaks today is described in H1.

**Nonce lifecycle.** Generation: `NonceChunk::with_size` (`frost/preprocess.rs:98-147`) samples 1024 `SigningNonces`, marshals `(D,E)`, builds leaves `keccak256(offset ‖ D.x ‖ D.y ‖ E.x ‖ E.y)` (`164-172`, matches `sol:FROSTNonceCommitmentSet.sol:147-159`), the tree (height 11, proof length 10 as required by `sol:…:131`) and stores each nonce with its proof. Registration: one SQLite transaction (`secrets/store.rs:141-167`), then `Action::Preprocess` (`state/preprocess.rs:22-50`), then `link(chunk, root)` on the own `Preprocess` event (`55-81`). Reveal: `nonces_reveal` (non-consuming, `store.rs:177-196`) → `Action::RevealNonceCommitments` with `expires_at = deadline` (`state/sign.rs:138-164`). Use: `take_nonce` (delete, `store.rs:205-218`) → `signature_share` → `Action::SignShare`. Ordering "delete before compute before broadcast" holds. Reorg handling: §6. Offsets below the contract's `startOffset` (`sol:…:98-104,129`) are not tracked locally: the validator may attempt a reveal that reverts (liveness only).

**Merkle (`merkle.rs`).** Sorted-pair keccak (`87-93`) identical to OpenZeppelin's commutative hashing used by all three contract verifiers. No leaf/node domain separation; second-preimage is prevented structurally: nonce leaves are keccak of 160 bytes and proofs must be exactly 10 long; selection leaves are keccak of 192 bytes (`frost/sign.rs:165-179` ⇔ `sol:FROSTSignatureShares.sol:118-133`, pinned by test `186-203`); participant leaves are the raw left-padded address word (`consensus/group.rs:244-247` ⇔ `sol:FROSTParticipantMap.sol:149`), so a forged leaf would need 96 leading zero bits. Odd levels are padded with `B256::ZERO` (`21-22`) consistently between `build` and `proof`. Group id = keccak of 4 words with the low 64 bits cleared (`consensus/group.rs:252-264` ⇔ `sol:FROSTGroupId.sol:36-60`). Group context = packed `(u32 version=0 ‖ consensus ‖ u64 epoch)` (`268-277`); genesis context = `keccak("genesis" ‖ salt)` or zero (`288-297`) — no Solidity counterpart (the contract treats context as opaque), so consistency is with deployment tooling.

**Signing (`frost/sign.rs`).** `signature_share` builds a `SigningPackage` over all revealed commitments keyed by address-derived identifiers (`69-76`), calls `round2::sign` (`82`), recomputes binding factors (`89-90`) and group commitment (`114-116`) with the `internals` API, derives each signer's `R_i = D_i + ρ_i·E_i` and Lagrange `λ_i` (`98-102`), publishes `(R_i, z_i, λ_i)` plus the selection `(R, root)` and proof. This matches the contract's `verifyShare` (`sol:FROST.sol:171-182`: `z_i·G = R_i + c·λ_i·Y_i` with `c = H2(R, Y, m)`) and `bindingFactors` (`100-128`, identifiers via `_hid`, commitment list ordered by identifier — `UnorderedCommitments` otherwise; the Rust side orders by identifier via `BTreeMap`). Message = 32-byte EIP-712 digest passed as raw bytes (`76`; contract hashes `abi.encode(message)` with H4, RFC-consistent). Onchain aggregation verifies each share; the validator never verifies peers' `z` (not needed).

**Message hashing (`consensus/hashing.rs`).** Domain `EIP712Domain(uint256 chainId,address verifyingContract)` (`95-103`) ⇔ `sol:ConsensusMessages.sol:15-18`; `EpochRollover(...)` type string ⇔ `21-24`; `TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)` with `oracleData` pre-hashed ⇔ `27-30,95-114`; `SafeTx` ⇔ `sol:SafeTransaction.sol:65-67,78-138`. The hand-rolled proposal encoding is pinned against alloy's `SolStruct` (`218-236`) and fixed vectors (`199-248`). `chain_id` comes from the RPC (`main.rs:53`).

**Marshal (`frost/marshal.rs`).** Scalars: `Scalar::from_repr` rejects ≥ n (`125-129`); points: `(0,0)` ⇒ identity, else SEC1 uncompressed decode (curve equation + coordinate range enforced by k256, `133-148`); signing commitments reject identity (`163-176`, ⇔ `sol:FROSTNonceCommitmentSet.sol:124-125`); `solidity_point(identity) = (0,0)` (`69-75`). Round-trip is canonical in both directions; no non-canonical encodings can reach frost-core.

**Zeroization / memory.** Explicit: `EncryptionKey` (drop), signer key bytes (`core/tx/signer.rs:60-63,89-92`). Implicit _(upstream, not read)_: frost-core `SigningNonces`, `SigningShare`, `SecretPackage`, `KeyPackage`. Not zeroized: `serde_json` strings holding secrets in the store paths (`store.rs:119,160,190,215`) and the snapshot JSON (core). The state snapshot persists `Secrets`, `SharingState.peer_packages` and `KeyShare` in plaintext (design; handbook §Consensus Secrets).

**Debug/Display.** Redacted: `Nonces`, `NonceChunk`, `EncryptionKey`, `Signer`. Derived and potentially printed: `Effect` (contains `Arc<KeyShare>`) at `warn` on any effect failure (`service/effect.rs:249`); `Resume::Setup{secrets}` / `Resume::Nonce` at `trace` (`core/effects.rs:59`, `core/driver.rs:261`). Safety depends on frost-core's `Debug` impls redacting `SigningShare`/coefficients — verify (checklist Q9).

**Constant-time.** All field/group arithmetic is k256/frost-core; the validator's own code only does XOR over bytes, public comparisons and `BTreeMap` lookups. Nothing secret-dependent branches in validator code.

---

## 10. Test coverage

35 tests (`#[test]`/`#[tokio::test]`) in 12 files: `config.rs` (4: parsing/defaults/sample), `consensus/group.rs` (5: thresholds, min participants incl. overflow, genesis vector, real onchain group + POAP), `consensus/hashing.rs` (4: Safe tx hash, packet hash, SolStruct parity, rollover hash), `consensus/epoch.rs` (1), `merkle.rs` (4), `frost/ecdh.rs` (4: roundtrip, commutativity, distinct recipients, identity rejection), `frost/participants.rs` (1), `frost/sign.rs` (1 leaf vector), `frost/preprocess.rs` (1 proof check on a real 1024 chunk), `frost/mod.rs` (1 end-to-end 3-party DKG + 2-of-3 signing incl. onchain-style aggregation), `secrets/store.rs` (6), `secrets/nonces.rs` (3).

**Untested:** the entire `state/` tree (0 tests: no timeout, restart, reorg, complaint, exclusion, rollover, oracle, attestation or top-up behaviour), `service/` (encoder gas/expiry, effect handler incl. `ReconcileGroupSecrets`), `frost/keygen.rs` negative paths (invalid PoK, wrong `y`, bad ciphertext, bad reveal, duplicate `q`), `frost/marshal.rs` edge cases (non-canonical scalars, off-curve points), `NonceState` arithmetic (`observe`/`reserve`/`available`), and every cross-implementation vector except the two pinned leaves. The flow-test epic (`epics/2026_07_14_validator_state_machine_flow_test_harness.md`) is not implemented (`flow_tests/` absent); its P0 matrix (restart during DKG/signing, burned-nonce reorg, complaint flows) is exactly where H3/H4/H7 live. The process-level integration test only covers the happy path with two validators.

---

## 11. Hypotheses

### H1 — DKG encryption key has no proof of possession; copying a peer's `q` plus the complaint flow leaks an honest participant's key share

- **Where:** `frost/ecdh.rs:110-121`; `frost/keygen.rs:79-100` (no `q` check beyond non-identity), `196-214` (encrypt with `(peer.q · sk_me).x`), `379-381` (decrypt likewise), `420-428` (`reveal_secret_share`); `state/keygen.rs:734-745` (automatic plaintext response to any complaint); `state/keygen.rs:714-731` (threshold counted per accused only); `sol:FROSTCoordinator.sol:377` (only `requireNonZero(q)`).
- **Excerpt (`frost/ecdh.rs:110-121`):**
  ```rust
  fn ecdh(sender_privkey: &NonZeroScalar, receiver_pubkey: &EncryptionPublicKey, msg: [u8; 32]) -> [u8; 32] {
      let shared_secret = (receiver_pubkey.0 * **sender_privkey).to_affine().x();
      let mut result = msg;
      for (byte, secret) in result.iter_mut().zip(shared_secret) { *byte ^= secret; }
      result
  }
  ```
- **Reasoning chain:** (1) `pad(X,Y) = (sk_X·sk_Y·G).x` depends only on the unordered pair of published keys; nothing binds sender/receiver identities and no PoP is required for `q`. (2) Malicious member `M` waits for honest `A`'s `KeyGenCommitted` and publishes its own valid polynomial/PoK but `q_M := q_A` (accepted by `verify_commitment` and by the contract). (3) In the sharing round, every honest `B_j` encrypts `f_{B_j}(M)` with `pad(B_j, q_M) = pad(B_j, A)`, and `A` encrypts `f_A(B_j)` with the same pad. (4) `M` files one complaint against each `B_j` and against `A` (contract: one per pair, plaintiff only needs `REGISTERED`); each honest validator answers unconditionally with the plaintext `f_{B_j}(M)` (`state/keygen.rs:734-745`). (5) `pad(B_j,A) = c_{B_j→M} ⊕ f_{B_j}(M)` is now public, decrypting the public ciphertexts `f_{B_j}(A)` and `f_A(B_j)` for all `j`. (6) `M` can now also _encrypt_ correctly to everyone using the learned pads (symmetry) and publishes valid shares, so nobody complains against `M`; per-accused totals stay at 1 (< threshold); `M`'s own complaints are all responded so it can confirm; the group **finalises with `M` as a member**. (7) `M` knows `f_A` at `n-1 ≥ t` points (`f_A(B_j)` for all `j`, plus `f_A(M)`), hence `a_0^A`, `f_A(A)`, and `s_A = f_A(A) + f_M(A) + Σ_j f_{B_j}(A)` — an honest member's complete signing share. With `m` colluders each copying a different honest `q`, they hold `2m` shares; `2m ≥ n/2+1` for `n = 7, 10, 13, …` at `m < n/3`, breaking the stated BFT guarantee; at `n = 6` (current testnet size, `consensus/group.rs:378-458`) one attacker holds half the threshold. Related keys (`q_M = q_A + G`, `k·q_A`) defeat a naive uniqueness check as well, since the x-coordinate of a known scalar multiple/offset reveals the base point up to sign.
- **Evidence class:** E2 (concrete input: commit with `q` copied from an earlier `KeyGenCommitted` event, then `keyGenComplain` against every peer).
- **Confidence:** 85%. Residual doubt: no local frost-core source; but the attack uses only validator-side code paths I read.
- **Confirm/refute:** Unit test with three `keygen::setup` results where the third's `encryption_key` is replaced by the first's public key; run `generate_secret_shares` for all, then show `verify_encrypted_secret_share`'s decrypted value for `(A, B)` equals `c_{B→A} ⊕ (c_{B→M} ⊕ f_B(M))`. Flow test on Anvil per the epic's 7B phase.
- **Severity:** Critical (key share leakage of an honest participant; BFT bound violated for n ≥ 7). Fix: derive the pad with a KDF bound to `(gid, sender, receiver)` and/or require a PoP for `q` (or reuse the PoK-covered `C[0]` as the docs describe).

### H2 — Coordinator/Consensus events are not bound to the emitting contract; an allow-listed oracle address can inject protocol events (worst case: forced plaintext share reveals)

- **Where:** `state/mod.rs:415-463` (only `OracleResult` receives `log.address`); `service/mod.rs:71-80` + `core/index/events.rs:568-591` (decode ignores address); `core/index/events.rs:405-408,459-463` (filter = any watched address × any watched topic); `main.rs:56-57` (watched = consensus, coordinator, all oracles); `state/keygen.rs:173-175` (commitments accepted from any participant address).
- **Excerpt (`state/mod.rs:428-430`):**
  ```rust
  Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplained(event)) => {
      self.handle_key_gen_complained(state, log.block, &event)
  }
  ```
- **Reasoning chain:** The watcher requests logs for `[consensus, coordinator, oracles…]` with the union of all selectors; a contract at an oracle address that emits `KeyGenComplained(gid, plaintiff=X, accused=me, false)` is decoded as a Coordinator event and, during a DKG, makes the validator queue `KeyGenComplaintResponse{secretShare: f_me(X)}` (`state/keygen.rs:734-745`). The transaction reverts onchain (`NotComplaining`) but the plaintext share is signed and broadcast (unless the core queue simulates first — `core/tx/mod.rs` not read; even then the value reaches the RPC provider). `t-1` such events with distinct plaintiffs (the `t`-th triggers a restart) reveal `t-1` points of every honest polynomial; with one colluding member, full polynomials. Other injectable events: fake `Sign` (advances `next_sequence`, burns local nonce coordinates, `state/sign.rs:30-34`), fake `SignRevealedNonces` for all signers (forces `UseNonce` and a share over an attacker-chosen commitment set — one use, then the nonce is gone), fake `KeyGenComplaintResponded` with a bad scalar (restarts the DKG excluding an honest accused, `state/keygen.rs:842-859`), fake `TransactionProposed`/`EpochStaged`.
- **Evidence class:** E2 for the missing check and the decode path; impact is conditional on a malicious or compromised contract in `config.validator.oracles`, which the configuration only trusts for approval results (`config.rs:66-69`).
- **Confidence:** 85% that the binding is absent and exploitable given the precondition; 30% that the precondition is realistic for the current deployment.
- **Confirm/refute:** Deploy a stub oracle emitting a Coordinator-shaped event on Anvil; observe the validator's queued `keyGenComplaintResponse` calldata. Refute if the core queue drops reverting transactions _and_ never sends them to the RPC.
- **Severity:** High (trust escalation from "approves transactions" to "controls the validator's protocol view"; impact up to Critical). Fix: check `log.address == coordinator`/`consensus` in `apply_transition` (the `Transition` currently lacks the coordinator address).

### H3 — Lost or failed `NonceTree` effects leave a phantom chunk reservation that is counted as capacity and never retried

- **Where:** `state/preprocess.rs:96-102` (reserve then effect), `199-203` (`reserve_chunk` inserts `None`), `234-247` (`available` counts `None` chunks), `180-194` (`observe` returns `None` for the phantom chunk); `state/keygen.rs:1307-1320` (chunk 0 reserved at finalize); `service/effect.rs:154-172` (`NonceTree` fails on `Unavailable`); `secrets/nonces.rs:53-59,194-218`; command order `state/mod.rs:470-479` (top-up effect spawned before the reconcile that starts the generator); `core/driver.rs:266-273`, `core/state/mod.rs:236,250-258` (effects non-durable).
- **Excerpt (`state/preprocess.rs:235-247`):**
  ```rust
  fn available(&self) -> u64 {
      let (chunk, offset) = preprocess::decode_sequence(self.next_sequence);
      self.chunks.range(chunk..).map(|(key, _)| {
          if *key == chunk { SEQUENCE_CHUNK_SIZE.saturating_sub(offset) } else { SEQUENCE_CHUNK_SIZE }
      }).sum()
  }
  ```
- **Reasoning chain:** A reservation is written to state (and committed with the block's logs) before the chunk exists. If the `NonceTree` effect never resumes — process restart while it is in flight (chunk generation + 1024 inserts takes seconds), or `Err(Unavailable)` because the process-local generator is empty after a restart and the same block's top-up/finalize effect runs before the reconcile effect starts the stream, or a worker thread that died (`secrets/nonces.rs:127-133`, never restarted because `start` is a no-op for existing entries `37-39`) — nothing re-issues it. `available()` keeps counting the phantom 1024 so `handle_nonce_topup` stays silent; every `Sign` whose sequence falls in that chunk yields `observe == None` ⇒ the session is dropped (`state/sign.rs:106-114`). The validator misses up to 1024 ceremonies (potentially the whole epoch at low traffic) and is repeatedly excluded by peers.
- **Evidence class:** E2 for the restart-ordering race (deterministic: the effect task calls `generator.next` immediately while the reconcile task first awaits two DB deletes); I for the crash window.
- **Confidence:** 65%.
- **Confirm/refute:** Restart a validator whose snapshot has a `None` reservation (or with `available() < 100` at the first block after restart); assert no `Preprocess` is ever emitted and that `Sign` events in the phantom chunk log "without a canonically linked nonce".
- **Severity:** High (liveness of the affected validator; network liveness if several restart, e.g. rolling upgrades). Fix: re-emit `NonceTree` for `None` reservations on `NewBlock`, or exclude `None` chunks from `available()`, and start generators before other effects.

### H4 — A lost `KeyGenSetup` resume during the genesis DKG stalls the validator (and therefore the network) indefinitely

- **Where:** `state/keygen.rs:1003-1024` (stuck detection requires `deadline: Some`), `41-54` (genesis deadline `None`), `1115-1154` (setup effect emitted from a Logs transition); `core/state/mod.rs:236-258` (resume not committed).
- **Excerpt (`state/keygen.rs:1004-1010`):**
  ```rust
  RolloverState::CollectingCommitments {
      next_epoch, group,
      secrets: KeyGenCommitment::Participating { secrets: None, .. },
      commitments,
      deadline: Some(deadline),
  } if block >= *deadline && commitments.len() as u16 == group.size().0 => {
  ```
- **Reasoning chain:** The genesis `KeyGen` event's block commits a snapshot with `secrets: None`; the setup effect resumes into live state only. Any restart before the next log-range commit (up to one block interval, ~5 s on Gnosis) reloads `secrets: None`, nothing re-spawns the effect, genesis has no deadline, so the validator never publishes its commitment; the contract needs all `count` commitments (`sol:FROSTCoordinator.sol:372`), so genesis never completes for anyone. Recovery requires deleting the snapshot table by hand (the secrets store would then be reused correctly).
- **Evidence class:** I (needs a restart in a ~1-block window; #851 explicitly left genesis without a deadline).
- **Confidence:** 55%.
- **Confirm/refute:** Flow test: start genesis, stop the node between the `KeyGen` log commit and the next block, restart; assert no `keyGenAndCommit` is ever sent.
- **Severity:** High (network-wide genesis liveness, manual recovery). Fix: on startup or `NewBlock`, re-issue `KeyGenSetup` whenever `Participating{secrets: None}` (the store makes it idempotent).

### H5 — Pairwise ECDH pads are two-time pads and unhashed x-coordinates, contradicting the documented one-time-pad argument

- **Where:** `frost/ecdh.rs:110-121`; `frost/keygen.rs:210-213,379-381`; `docs/overview.md` §KeyGen ("each ECDH shared secret is used to encrypt exactly one value").
- **Excerpt:** see H1.
- **Reasoning chain:** `c_{A→B} ⊕ c_{B→A} = f_A(B) ⊕ f_B(A)` is public for every honest pair. XOR of two independent uniform scalars does not by itself reveal either, and the relation is non-linear over F_n, so no direct recovery is known; but the security argument in the docs is false as stated and the construction lacks the standard KDF step (also leaking ~1 bit via abscissa validity).
- **Evidence class:** E2 (relation directly computable from public events).
- **Confidence:** 90% that the property is violated; 15% that it is exploitable on its own.
- **Confirm/refute:** Assert `ecdh(a, B, x) == ecdh(b, A, x)` (already the `ecdh_is_commutative` test) and note both directions use it.
- **Severity:** Medium (design gap; becomes Critical only in combination with H1). Fix as in H1.

### H6 — Permissionless `Coordinator.sign` lets anyone burn every validator's committed nonce sequence; sessions hit by an unlinked sequence are dropped for good

- **Where:** `state/sign.rs:30-34` (observe before matching), `106-114` (session removed and not re-inserted), `state/preprocess.rs:180-194`; `sol:FROSTCoordinator.sol:530-542` (no access control on `sign`).
- **Excerpt (`state/sign.rs:106-114`):**
  ```rust
  (None, Some(SigningState::WaitingForRequest { .. })) => {
      tracing::warn!(... "not participating in signing request without a canonically linked nonce");
  }
  ```
- **Reasoning chain:** Every `Sign` for a tracked group advances `next_sequence` and prunes older chunks. An attacker calling `sign(gid, junk)` ~100× per block (150k gas each) drains 1024-nonce chunks faster than the eager generator can comfortably replace them, forces continuous preprocessing (CPU + gas) and, whenever a real `Sign` lands in a chunk not yet linked, the validator drops the session and ignores all later restarts of that message (`WaitingForRequest` is only recreated by a fresh `TransactionProposed`, `state/transactions.rs:52-73`). Rollover messages are only recreated at the next confirmation.
- **Evidence class:** E2 (observable with a script calling `sign` in a loop on Anvil).
- **Confidence:** 70%.
- **Confirm/refute:** Flow test: spam `sign` so that a proposal's sequence lands beyond the linked chunks; assert the validator never rejoins after the responsible party restarts.
- **Severity:** Medium (griefing/liveness; cost to attacker is gas only). Fix: keep the session (re-insert it) when no nonce is linked; consider contract-side access control on `sign`.

### H7 — `ReconcileGroupSecrets` races store writes of effects spawned by the same block's logs

- **Where:** `service/effect.rs:202-238` (two unconditional `DELETE … NOT IN` computed from stale state), `core/effects.rs:53-62` (concurrent tasks), `state/mod.rs:464-481` (reconcile from `NewBlock` state), `state/keygen.rs:41-54,486-506,599-607` (DKG/epoch started from Logs transitions), `secrets/store.rs:232-253`.
- **Excerpt (`service/effect.rs:226-229`):**
  ```rust
  self.secrets.retain_nonces(keygen.iter().copied().chain(nonces.keys().copied())).await?;
  self.secrets.retain_keygen_secrets(keygen).await?;
  ```
- **Reasoning chain:** The reconcile for block N excludes any group first tracked by block N's logs (e.g. genesis `KeyGen`, the next-epoch DKG started at final confirmation, the epoch registered at `EpochStaged`). If its `DELETE` executes after that group's `store_keygen_secrets`/`register_nonces_chunk` (both effects run concurrently on a 10-connection pool), the freshly stored secrets/nonces vanish while the state machine believes they exist. Symptoms: a reorg/restart replay then samples _new_ DKG secrets ⇒ `IncorrectCommitment` ⇒ epoch skipped / genesis `Halted`; or a linked nonce root with no rows ⇒ silent non-participation for a chunk.
- **Evidence class:** I (timing-dependent; no deterministic trigger found).
- **Confidence:** 30%.
- **Confirm/refute:** Inject a delay into `retain_groups` in a test and run the genesis flow; check `keygen_secrets` after the `KeyGen` block.
- **Severity:** Medium (rare, but genesis outcome is a permanent halt). Fix: run reconciliation from a state that already includes the block's log effects, or make retention decisions and inserts serialise through one task.

### H8 — Re-revealing nonces lets a signer appoint itself "responsible" and stall each ceremony by two timeouts

- **Where:** `state/sign.rs:282-284` (`last_signer = event.participant` on every accepted reveal), `522-562,577-616` (responsible-only restart, then everyone after another timeout); `sol:FROSTCoordinator.sol:554-558` (no per-participant reveal dedupe).
- **Reasoning chain:** `M` reveals early, then re-reveals the same leaf after everyone else; on timeout `M` is `last_signer`, does nothing, and the round waits another `signing_timeout` before all signers restart without `M`. Bounded griefing (~12 blocks per ceremony) and `M` is excluded afterwards.
- **Evidence class:** E2. **Confidence:** 60%. **Severity:** Low.

### H9 — Complaint/response deadline asymmetry can exclude honest validators after a late complaint

- **Where:** `state/keygen.rs:692-695` (complaints after `deadlines.complain` ignored, so the honest accused never responds), `1083-1090`, `sol:FROSTParticipantMap.sol:219-225` (a plaintiff with open complaints cannot confirm).
- **Reasoning chain:** A late complaint is accepted onchain but ignored offchain by everyone including the accused; the plaintiff can never confirm; the round fails at `confirm` and _the plaintiff_ (honest if the share was genuinely bad but late) is excluded. Deterministic across honest nodes, so no divergence — but a malicious member gets a cheap way to waste a whole DKG round by complaining late. **Evidence class:** E2. **Confidence:** 50%. **Severity:** Low.

### H10 — Secret-bearing structs derive `Debug` and are logged at `warn` on effect failure

- **Where:** `service/effect.rs:24-62` (`Effect` derives `Debug`, carries `Arc<KeyShare>`), `249` (`warn!(?effect …)`), `frost/keygen.rs:27,144,302,433`.
- **Reasoning chain:** Any transient DB error while reconciling prints every tracked `KeyShare`; safety depends entirely on frost-core's `Debug` redaction of `SigningShare` _(upstream, not read)_. **Evidence class:** I. **Confidence:** 25% that secrets actually print. **Severity:** Critical if upstream does not redact, else Info. Cheap to make robust with a manual `Debug`.

### H11 — Restoring the SQLite backup can resurrect consumed nonces

- **Where:** `secrets/store.rs:198-218` (deletion is the only reuse guard); handbook `docs/validator-handbook.md:19,75` recommends backups. A restore replays the same chain and normally recomputes identical shares, but after a reorg or a ceremony restart in the interval the resurrected nonce can be signed over a different package. **Evidence class:** I. **Confidence:** 40%. **Severity:** Medium (operational; documentation fix + a "nonce reuse guard" keyed by sequence would close it).

### Considered and rejected

- **Marshal canonicality:** scalars ≥ n rejected (`marshal.rs:125-129`); points validated by k256; identity handled consistently with `sol:Secp256k1.sol:207-215`.
- **Identifier / leaf / group-id / EIP-712 mismatches:** all encodings match the Solidity libraries cited in §9; two vectors pinned by tests, others checked by reading.
- **Nonce reuse across reorgs through the state machine:** prevented by store deletion (§6).
- **Stale `Resume::Nonce` applied to a restarted session:** blocked by frost-core's own-commitment check (`IncorrectCommitment`) _(upstream, not read)_ — flagged in checklist Q4 rather than as a finding.
- **Selection-root ambiguity on timeout:** unique because threshold > n/2 and `max_by_key` over a `BTreeMap` is deterministic (`state/sign.rs:677-683`).
- **Merkle second-preimage / zero-padding:** structurally infeasible (§9).
- **`as u16` truncation:** bounded by contract membership; only reachable with H2.
- **Lagrange coefficients over address identifiers:** computed by frost-core and verified onchain; a wrong `l` only breaks the submitter's own share.
- **Rayon blocking the runtime:** not the case (dedicated thread).
- **Complaint tainting threshold off-by-one:** `>=` on both sides; `t-1` legitimate reveals never give an outsider `t` points of one polynomial without H1.

---

## 12. Suggested review checklist (ordered by risk)

1. **ECDH key binding (H1, H5):** In `frost/ecdh.rs` and `frost/keygen.rs:196-214,353-383`, can a participant publish a `q` it does not own (copied or scalar-related) and still finalise membership by learning pads through complaints? Is a PoP for `q` or a KDF bound to `(gid, sender, receiver)` required? Reconcile `docs/overview.md` §KeyGen with the implementation.
2. **Event address binding (H2):** Does `state/mod.rs:415-463` (and `core/index/events.rs` decoding) verify that Coordinator/Consensus events come from the coordinator/consensus addresses? What is the blast radius of one malicious address in `config.validator.oracles`? Does `core/tx/mod.rs` broadcast transactions that would revert?
3. **Effect durability across restarts (H3, H4):** For each `Effect` variant (`service/effect.rs:25-62`), what happens if the process dies between the snapshot commit that emitted it and its resume? Verify `NonceState` phantom reservations (`state/preprocess.rs:199-247`) and genesis `secrets: None` (`state/keygen.rs:1003-1024`) are recoverable without manual DB surgery. Check startup ordering of `NonceTree` vs `ReconcileGroupSecrets` (`state/mod.rs:470-479`, `secrets/nonces.rs:53-59`).
4. **Nonce single-use under all resume orderings:** Confirm frost-core 3.0.0 `round2::sign` rejects a signing package whose own commitment differs from `SigningNonces` (protects `state/sign.rs:359-404` against stale resumes), and that `Nonces`/`SigningNonces` zeroize on drop.
5. **Sequence griefing (H6):** Evaluate `Coordinator.sign` being permissionless against `state/sign.rs:30-34,106-114`; decide whether dropped sessions should survive restarts of the same message and whether the top-up threshold (`state/preprocess.rs:17`) and eager generation keep up under adversarial sequence consumption.
6. **Reconciliation race (H7):** Trace the concurrency of `ReconcileGroupSecrets` (`service/effect.rs:202-238`) against `store_keygen_secrets`/`register_nonces_chunk` for groups introduced by the same block's logs.
7. **Complaint-flow policy:** Per-accused threshold only (`state/keygen.rs:714-731`); unlimited complaints per plaintiff; unconditional plaintext responses (`734-745`); late-complaint asymmetry (`692-695`, H9). Is a per-plaintiff bound or a challenge/response that does not reveal the share feasible?
8. **Config trust:** `participants` is local per validator (`consensus/group.rs:177-217`) — audit how operators keep it consistent; `genesis_salt`/context derivation (`268-297`) has no onchain counterpart; `oracles` default empty.
9. **Secret hygiene in logs and snapshots:** Verify frost-core `Debug` redaction for `KeyPackage`, `SigningShare`, `round1/round2::SecretPackage` (H10); enumerate what the `snapshots` table stores (`State` incl. `SharingState.peer_packages`, `KeyGenCommitment::Participating.secrets`) and its pruning; consider a manual `Debug` for `Effect`/`Resume`.
10. **Backup/restore semantics (H11):** Document that restoring `validator.db` after a reorg or after any signing activity can reuse nonces; consider recording consumed `(group, sequence)` pairs in an append-only table.
11. **Gas constants (`service/action.rs`):** Validate each fixed limit against Foundry gas reports for the largest expected group (e.g. `keyGenAndCommit` with `threshold` points at 250k, `signShareWithCallback` 400k incl. `attestTransaction`/`stageEpoch` callback and rollover processing).
12. **Timeout determinism:** Compare `state/keygen.rs:132-133` (late setup deadline) with `251-252`, and all `deadlines` derivations, for divergence between honest nodes that observed the same chain but resumed effects at different times.
13. **Reorg depth exit (#834):** The safe block hash is not persisted (commit message); a reorg coinciding with a restart is undetected — what does the validator do with a stale snapshot on a different branch (`core/state/mod.rs:135-143`)?
14. **Test debt:** No tests for `state/`, `service/`, DKG negative paths, or marshal edge cases; implement the epic's P0 flows (genesis restart, burned-nonce reorg, complaint corruption) before mainnet.
