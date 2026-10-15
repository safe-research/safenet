# L-crypto coverage log (Rust audit run 3, blind discovery track)

| Field | Value |
| --- | --- |
| Agent | L-crypto (crypto-expert department), cryptography lens across `core`, `validator`, `sentinel` |
| Commit | `361aaf3` (main checkout read for citations; builds and tests in `<run folder>/wt/base`, also `361aaf3`, clean) |
| Date | 2026-10-08 |
| Findings | `F3-XC-100` (High, E1 for the mismatch and the exclusion arithmetic, E2 for the state-machine glue) |
| Evidence logs | `rust-audit/state/run3/logs/L-crypto-identity-share.txt`, `rust-audit/state/run3/logs/L-crypto-parity.txt` |
| Scratch proofs | `<run folder>/scratch/L-crypto/identity-share/` (Rust crate compiling the validator's `frost/`, `merkle.rs`, `bindings.rs`, `consensus/{group,epoch,hashing}.rs` by path, read-only) and `<run folder>/scratch/L-crypto/forge-identity/` (Foundry project remapped to the worktree's `contracts/src`). Nothing tracked was edited |

`<run folder>` is the local run folder.

## Independence

I read only `rust-audit/PROMPT.md`, `state/run3/{settled,map,baseline}.md`, the shared brief, the maintainers' project notes and the team's decisions, the code and the contracts. `git status` in the main checkout listed modified file names under `rust-audit/findings/` (informed track); I did not open them. `ls` of `state/run3/agents/` and `<run folder>/scratch/` showed `rv-*` file names; I did not open them. I did not read PR #905.

## Files

All 15 assigned files read in full, tests included.

| File                                          | Lines | Read  |
| --------------------------------------------- | ----: | ----- |
| `crates/validator/src/frost/mod.rs`           |   325 | 100 % |
| `crates/validator/src/frost/keygen.rs`        |   515 | 100 % |
| `crates/validator/src/frost/sign.rs`          |   236 | 100 % |
| `crates/validator/src/frost/ecdh.rs`          |   316 | 100 % |
| `crates/validator/src/frost/marshal.rs`       |   181 | 100 % |
| `crates/validator/src/frost/serialization.rs` |    85 | 100 % |
| `crates/validator/src/frost/participants.rs`  |    33 | 100 % |
| `crates/validator/src/frost/error.rs`         |    46 | 100 % |
| `crates/validator/src/merkle.rs`              |   142 | 100 % |
| `crates/validator/src/consensus/group.rs`     |   459 | 100 % |
| `crates/validator/src/consensus/hashing.rs`   |   249 | 100 % |
| `crates/validator/src/state/sign.rs`          |   836 | 100 % |
| `crates/core/src/kdf.rs`                      |    81 | 100 % |
| `crates/core/src/tx/signer.rs`                |   297 | 100 % |
| `crates/sentinel/src/hashing.rs`              |   224 | 100 % |

Context read (not assigned, for tracing): `crates/validator/src/state/keygen.rs` (all 1,447 lines: DKG flow, complaints, timeouts, restarts), `crates/validator/src/state/transactions.rs` (all), `crates/validator/src/service/effect.rs` (all), `crates/validator/src/service/action.rs:100-353`, `crates/validator/src/service/mod.rs:30-80`, `crates/validator/src/secrets/store.rs:1-426`, `crates/validator/src/bindings.rs` (all), `crates/validator/src/consensus/epoch.rs:1-40`, `crates/validator/src/config.rs:90-108`, `crates/sentinel/src/service.rs:205-253, 420-450`, `crates/sentinel/src/bindings.rs:80-165`. Contracts (reference): `libraries/{FROST,Secp256k1,FROSTSignatureShares,FROSTSignatureId,FROSTParticipantMap,FROSTGroupId,ConsensusMessages,SafeTransaction}.sol` (all), `SentinelOracleCommitments.sol:30-131`, `FROSTCoordinator.sol` (all), `Consensus.sol:148-398`, `Safenet7702Executor.sol:60-140`, `script/util/Genesis.sol`. Library sources (registry): frost-core 3.0.0 `keys/dkg.rs` (challenge, PoK, part2, part3), `keys.rs` (`from_commitment`, `evaluate_vss`, `SecretShare::verify`, Debug impls), `round2.rs:125-170`, `verifying_key.rs:83-93`; frost-secp256k1 3.0.0 `lib.rs:60-235`.

## Commands

| Command | Result |
| --- | --- |
| `source <run folder>/env/base.sh && cargo test -p validator --bins -- frost:: merkle:: consensus::` | 25 passed, 0 failed |
| `source <run folder>/env/base.sh && cargo test -p safenet-core --lib -- kdf:: tx::signer::` | 9 passed, 0 failed |
| `source <run folder>/env/base.sh && cargo test -p sentinel --bins -- hashing::` | 5 passed, 0 failed |
| `cd <run folder>/scratch/L-crypto/identity-share && CARGO_TARGET_DIR=$PWD/target cargo test --offline poc -- --nocapture` | 1 passed: victim publishes `y = (0, 0)`; exclusion `{attacker, victim}` is not viable |
| `cd <run folder>/scratch/L-crypto/forge-identity && forge test -vv --match-contract IdentityShareTest` | 1 passed: victim's validator-encoded share reverts `NotOnCurve` |
| `cargo test --offline ceremony -- --nocapture` (same crate) | 2 passed: Aegis-size DKG and 3- and 5-signer vectors written |
| `forge test -vv --match-contract CeremonyTest` | 1 passed: DKG finalizes onchain, both ceremonies complete and verify |
| `cargo test --offline hashing_vectors -- --nocapture` and `forge test -vv --match-contract ParityTest` | 1 and 5 passed: every Rust hashing vector equals the Solidity libraries |
| `git show fe9e84c:<path>` for `FROSTParticipantMap.sol`, `frost/keygen.rs`, `frost/marshal.rs`, `state/keygen.rs`, `frost/mod.rs`; `git show 7fc1e08` | pre-existence of `F3-XC-100` |

## Checklist answers

- Q62 (reveal salt): `derive_key` is HKDF-SHA256 with the private key as IKM, the constant `safenet-sentinel-reveal-salt` as salt and the request id as info (`crates/core/src/kdf.rs:19-27`, `crates/core/src/tx/signer.rs:127-132`, `crates/sentinel/src/hashing.rs:14, 49-53`). Revealing outputs reveals neither the PRK nor the key (PRF security); outputs for future request ids stay unpredictable, which is all the commitment's hiding needs. The only other use of the key is ECDSA (transactions, 7702 authorizations, RFC 6979 nonces), whose HMAC keys differ from the HKDF extract key, so no cross-protocol relation. `derive_key` has one caller (`grep`). Sound.
- Q63 (ECDH pad reuse): restarts use a new group id, so `store_keygen_secrets` samples a fresh encryption key (`secrets/store.rs:172-208`); a same-id retry reuses identical secrets, so identical ciphertexts. Beyond the known two-direction symmetry, a complaint response publishes the pair's pad (the revealed `f_j(i)` XOR the posted ciphertext), so the plaintiff's own share to the accused becomes public. Not exploitable: an honest-honest complaint cannot occur (verification is deterministic over the same verified commitment map), and in a pair with a malicious party it only publishes a value that party already holds. Observation O1, tagged `known`.
- Q64: `hash_to_scalar(b"enc", entropy)` uses the DST `FROST-secp256k1-SHA256-v1enc`, distinct from frost-secp256k1's `rho`, `chal`, `nonce`, `dkg`, `id`, `randomizer` (`frost-secp256k1-3.0.0/src/lib.rs:188-234`); its input is fresh entropy anyway. The PoP challenge is `HDKG(id || q || R)` with the address-derived identifier, identical to `FROST.keyGenChallenge`; executed: `keyGenCommit` accepted the validator's PoK and PoP for 5 participants (`logs/L-crypto-parity.txt`).
- Q65: the leaf uses `R_i = D_i + rho_i E_i` and `l_i = derive_interpolating_value` over the committed set, laid out as `FROSTSignatureShares._hash`; the binding factor round trip is exact (32-byte big-endian scalar both ways). Executed: 3- and 5-signer ceremonies from the validator's `signature_share` pass `verifyShare`, the Merkle proof and `FROST.verify` in the real coordinator.
- Q66: sorted pairs and zero padding match OpenZeppelin `MerkleProof` (executed with 3 and 5 leaves, odd levels). Participant leaves are raw address words, but the contract computes the leaf from `msg.sender`; a node-as-leaf confusion would need an internal node with 96 leading zero bits, and the zero padding leaf is `address(0)`, which cannot send. Sound.
- Q67: the domain `EIP712Domain(uint256 chainId,address verifyingContract)`, both typehashes, the hand-rolled proposal encoding, `SafeTx` hash and partial hash all equal the Solidity libraries; executed with the repository vectors and a non-trivial vector (every `SafeTx` field distinct, DELEGATECALL, non-empty data and oracle data, chain 100), with a control showing a swapped field changes the hash.
- Q68: nonces are `round1::SigningNonces::new(signing_share, thread_rng)`, the RFC 9591 hedged construction; adequate. A serde copy exists only in the store (written once, read by `take` once; `store_signing_nonces` deserializes a second copy only to compute commitments); frost types zeroize on drop (`round1.rs:212`). No path uses a copy to sign twice.
- Q69: the authorization is for the provider's chain id (0 refused, `tx/signer.rs:61-63`), the configured executor and `nonce + 1`, which EIP-7702 requires for a self-sponsored transaction; k256 signs low-s. Bound tightly enough; the executor-code check is settled L1.
- Q26: PoK and PoP share one challenge, so a PoK is a valid PoP only when `q == c[0]` of the same participant: self-harm only. A copied or related `q` (for example `-q_H`, which would give the same x-coordinate pad) needs a PoP under the copier's identifier, so it needs the victim's key; checked both sides (`FROSTCoordinator.sol:375`, `frost/ecdh.rs:115-126`).
- Q27: publisher and recipient both index by the same verified commitment map in ascending address order without the sender (`frost/keygen.rs:204-212, 364-373`); holds for every size and exclusion set.
- Q28: the group key cannot be the identity (PoK on each `c[0]`). A verifying share can: frost-core 3.0.0 `VerifyingShare::from_commitment` is infallible and `VerifyingKey::from_commitment` does not check identity. The contract rejects an identity `y`, which is `F3-XC-100`.
- Q33: nonces taken for one signature id and applied to another id's committed set fail frost-core's commitment check (`round2.rs:141-143`), so the result is only a missing share; the nonces are burned.
- Q34: single use holds: `take_signing_nonces` clears under `BEGIN IMMEDIATE` (`secrets/store.rs:266-294`), a used id keeps a `NULL` row and never gets fresh nonces (`:220-256`), and reorgs that remap an id to another message still consume each pair once.
- Q35: every validator applies `NewBlock(deadline)` before the deadline block's logs, live and in warps, so the continued signer set is identical.
- Q36: honest signers compute one root per signature id from the same committed set; a misbehaving signer can register shares only under roots of its own making, which cannot reach the threshold without honest shares; ties are broken deterministically by `BTreeMap` order. A split of honest shares needs a reorg (O6).

## Rejected hypotheses

| # | Hypothesis | Refuted by |
| --- | --- | --- |
| R1 | EIP-712 domain, typehash or field encoding differs from Solidity | `logs/L-crypto-parity.txt` (repository and non-trivial vectors) |
| R2 | Selection leaf, Lagrange coefficient or binding factor differs from `FROST.verifyShare` | `logs/L-crypto-parity.txt` (Aegis-size ceremonies complete onchain) |
| R3 | Odd-level zero padding differs from OpenZeppelin `MerkleProof` | same (3 and 5 leaves); `consensus/group.rs:377-458` real Gnosis vector |
| R4 | Participant-tree leaf and node confusion (unhashed address leaves) | leaf derived from `msg.sender` (`FROSTParticipantMap.sol:149-150`); 96 zero bits needed |
| R5 | PoK or PoP challenge differs from `FROST.keyGenChallenge` | onchain `keyGenCommit` accepted the validator's proofs (`logs/L-crypto-parity.txt`) |
| R6 | A participant reuses or negates another's encryption key to read shares | PoP checked onchain and offchain (`FROSTCoordinator.sol:375`, `frost/ecdh.rs:115-126`) |
| R7 | Group key forced to the identity or to `x >= N` | PoK binds `c[0]`; grinding `x >= N` is about 2^128 work |
| R8 | Verifying share forced to `x >= N` (rejected by `mulmuladd`) | needs an unknown-log coefficient, so the attacker's shares fail and complaints exclude it alone; `set` accepts the victim's on-curve `y` |
| R9 | Off-curve `c[i]` for `i >= 1` accepted onchain but rejected in Rust splits state | Rust ignores the commitment and the commitment deadline excludes the sender alone (`state/keygen.rs:1043-1060`); same as a withheld commitment |
| R10 | Nonce reuse after restart, reorg or signature-id remap | `round2.rs:141-143`, `secrets/store.rs:220-294` |
| R11 | Secrets reach logs through `Debug` (`Effect`, `Resume`, `Secrets`, `SharingState`, `KeyShare`, `VerifiedShare`, `SigningNonces`) | redacting `Debug` in frost-core (`keys.rs:126-132`, `round1.rs:271-278`, `keys/dkg.rs:191-198, 337-345`) and `frost/ecdh.rs:71-75`, `frost/sign.rs:49-53` |
| R12 | Sentinel salt predictable or related to ECDSA nonces | HKDF extract key differs from RFC 6979's HMAC keys; PRF output |
| R13 | Honest-honest complaint through index divergence | same verified map on both sides (`frost/keygen.rs:204-212, 364-373`) |
| R14 | Attacker-triggered error in `generate_secret_shares` or `finalize`, which would skip the epoch for everyone | every error path enumerated: count, length (contract-enforced), PoK (already verified), own-secret consistency |
| R15 | Group id or genesis context differs from `FROSTGroupId.create` and `script/util/Genesis.sol` | `ParityTest.test_GroupVectors`; the identity proof asserts the coordinator's returned id |
| R16 | Shared coordinator between Aegis production and the testnet reboot collides genesis group ids | different salt and participants (`contracts/.env.testnet.sample:11-12`, `contracts/.env.aegis.sample:10-11`) |
| R17 | The last signer tampers with the callback context | `Consensus.attestTransaction` and `stageEpoch` recompute the message and call `signatureVerify` (`Consensus.sol:229-248, 272-288`) |
| R18 | Stale `signature_id_to_message` entries mix an old ceremony's events into a new one (the handlers do not compare the state's id with `event.sid`) | every transition that retires an id removes its entry (`state/sign.rs:209, 472, 563, 583, 634, 686`); no stale path found |
| R19 | The identity-tolerant serde lets a malformed snapshot point through | input is the validator's own snapshot (A1, A17); k256 `AffinePoint` deserialization checks the curve |

## Observations (plausible, not proven or not exploitable)

- O1 (`known`, ECDH pad): a complaint response makes the pair's pad public, so the plaintiff's share to the accused is published too. Within the threat model it publishes only values a malicious party in the pair already holds (Q63).
- O2 (hardening): the DKG PoK and PoP challenges carry no group, epoch or chain context and share one hash domain (frost-core design, mirrored in `FROST.sol:213-246`). Identifier binding makes replay harmless; separating the PoP domain would need a contract change.
- O3 (A1, hardening): a taken nonce pair is set to `NULL` in SQLite (`secrets/store.rs:283`) without `secure_delete`, so the old value can survive in free pages or the WAL; once the share is public, a recovered pair yields the signing share. The plaintext key share in the snapshot already exposes more.
- O4 (A1, hygiene): the HKDF PRK and HMAC state for the reveal salt are not zeroized after use (`kdf.rs:22-26`).
- O5 (for R5): a reorg within `max_reorg_depth` can land a `SignShare` computed on the dropped branch, splitting honest shares across roots; if no root then reaches the threshold, the share-round timeout drops the ceremony (`state/sign.rs:623-643`, the `return false` at `:642`) and only a re-proposal recovers. Liveness only.
- O6 (for R5, Q37): queued `CommitNonces` and `SignShare` actions computed after an approving `OracleResult` are not rolled back, so a signature can complete on a branch where that result was reorged out.
- O7 (map note): `consensus/hashing.rs` hand-rolls the proposal encoding; it is pinned against `SolStruct` and now against Solidity with non-trivial values, so no defect.
