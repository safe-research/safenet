//! PoC part 1 for F-VAL-005 and F-VAL-066 — the *consequence* half.
//!
//! Once `retain_keygen_secrets` has deleted a group's row, `store_keygen_secrets`
//! is no longer insert-only-with-reuse: it resamples, and the resampled
//! polynomial no longer matches the commitment already onchain, so
//! `generate_secret_shares` fails with `IncorrectCommitment` — the error both
//! findings terminate in. The nonces half (F-VAL-066 (b)) is here too.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::secrets` by adding to
//! `crates/validator/src/secrets/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-005-066/secrets_reconciliation.rs"]
//! mod poc_f_val_005_066;
//! ```

use super::store::SecretStore;
use crate::frost::keygen::{self, KeyShare};
use crate::frost::preprocess::NonceChunk;
use alloy::primitives::{Address, B256, address};
use sqlx::sqlite::SqlitePool;
use std::collections::BTreeMap;

const GROUP: B256 = B256::repeat_byte(0xa1);
const OTHER_GROUP: B256 = B256::repeat_byte(0xb2);
const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEERS: [Address; 2] = [
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
];
const COUNT: u16 = 3;
const THRESHOLD: u16 = 2;

async fn store() -> SecretStore {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    SecretStore::new(pool).await.unwrap()
}

/// **The consequence chain, end to end, with no state machine.**
///
/// Mirrors what `Effect::ReconcileGroupSecrets` does when the retention set
/// omits an in-progress DKG (`service/effect.rs:207-229` →
/// `store.rs:231-253`), then what `Effect::KeyGenSetup` does on the replay
/// (`service/effect.rs:131-138`), then what
/// `finalize_key_gen_commitments` does with the result
/// (`frost/keygen.rs:177-183`).
///
/// PASS ⇒ every link holds:
///   1. `store_keygen_secrets` honours "never overwritten" while the row exists;
///   2. `retain_keygen_secrets` with a set that omits the group deletes it anyway;
///   3. the next `store_keygen_secrets` therefore returns **different** secrets
///      with a **different** commitment;
///   4. `generate_secret_shares` with those secrets against the commitment set
///      containing the *original* published commitment fails with
///      `IncorrectCommitment`.
/// That is the whole of F-VAL-005's steps 3-7 and F-VAL-066's final paragraph,
/// reduced to a deterministic test.
///
/// FAIL at (2) ⇒ the delete does not reach a group in DKG; both findings lose
/// their mechanism.
/// FAIL at (3) ⇒ two independent `keygen::setup` calls produced the same
/// commitment, which cannot happen with a working RNG — investigate the RNG,
/// not the finding.
/// FAIL at (4) ⇒ `generate_secret_shares` tolerates the mismatch; the ceremony
/// would then proceed with inconsistent material, which is *worse*, not better.
/// Report it as a new finding.
#[tokio::test]
async fn deleting_the_row_makes_the_resample_incompatible_with_the_published_commitment() {
    let store = store().await;

    // (1) The original ceremony. `Effect::KeyGenSetup` samples and persists.
    let original = keygen::setup(&mut rand::thread_rng(), ME, COUNT, THRESHOLD).unwrap();
    let stored = store
        .store_keygen_secrets(GROUP, ME, original.clone())
        .await
        .unwrap();
    let published_commitment = stored.commitment();
    assert_eq!(
        published_commitment,
        original.commitment(),
        "the store returns what it was given for a fresh group"
    );

    // The documented invariant holds *while the row exists*: a replayed effect
    // reuses the retained secrets.
    let replayed = store
        .store_keygen_secrets(
            GROUP,
            ME,
            keygen::setup(&mut rand::thread_rng(), ME, COUNT, THRESHOLD).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        replayed.commitment(),
        published_commitment,
        "store.rs:101-104's 'never overwritten' invariant, confirmed"
    );

    // (2) *** THE DEFECT ***
    // The reconciliation runs with a retention set that does not name this
    // group — which is what `handle_group_reconciliation` produces whenever
    // `state.rollover` does not currently point at it
    // (`state/preprocess.rs:130-165`, the `_ => None` arm). After a reorg to a
    // block before the ceremony started, that is exactly the state, and
    // `Message::NewBlock` runs before that block's logs
    // (`crates/core/src/state/mod.rs:182-199`).
    //
    // Note that `OTHER_GROUP` here stands for "some other group is retained";
    // with the empty set the statement degrades to a bare
    // `DELETE FROM keygen_secrets`, which is F-VAL-066's wider case and is
    // covered by `an_empty_retention_set_wipes_the_whole_table` below.
    store.retain_keygen_secrets([OTHER_GROUP]).await.unwrap();

    // (3) The replayed `KeyGenSetup` now inserts rather than reuses.
    let resampled = store
        .store_keygen_secrets(
            GROUP,
            ME,
            keygen::setup(&mut rand::thread_rng(), ME, COUNT, THRESHOLD).unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        resampled.commitment(),
        published_commitment,
        "F-VAL-005 step 4: the row was deleted, so the setup resampled"
    );

    // (4) The ceremony now fails at the guard FROST does not perform itself.
    // Build the commitment set the validator sees onchain: the *original*
    // commitment for `ME` (re-included by the reorg) plus the peers'.
    let mut commitments = BTreeMap::new();
    commitments.insert(
        ME,
        keygen::verify_commitment(ME, &published_commitment).unwrap(),
    );
    for peer in PEERS {
        let peer_secrets = keygen::setup(&mut rand::thread_rng(), peer, COUNT, THRESHOLD).unwrap();
        commitments.insert(
            peer,
            keygen::verify_commitment(peer, &peer_secrets.commitment()).unwrap(),
        );
    }

    let error = keygen::generate_secret_shares(resampled, commitments)
        .expect_err("F-VAL-005: the resampled secrets cannot serve the published commitment");
    // `frost/keygen.rs:179-183` returns `Error::IncorrectCommitment`, which
    // `err_unexpected` wraps as `frost::error::Error::Unexpected`.
    assert!(
        matches!(
            error,
            crate::frost::error::Error::Unexpected(frost_secp256k1::Error::IncorrectCommitment)
        ),
        "expected IncorrectCommitment, got {error:?}"
    );
}

/// **F-VAL-066's wider blast radius.** With an empty retention set — which is
/// what `handle_group_reconciliation` produces on *every* block while
/// `rollover` is `WaitingForGenesis` and `state.epochs` is empty —
/// `retain_groups` builds a bare `DELETE FROM <table>`
/// (`store.rs:236-242`).
///
/// PASS ⇒ the unqualified wipe is real and hits every group at once, including
/// groups the current block's logs are about to introduce. This is the increment
/// F-VAL-066 owns over F-VAL-005.
///
/// FAIL ⇒ the empty case is guarded somewhere; F-VAL-066's specific claim (b)
/// is refuted and only F-VAL-005's narrower `WHERE group_id NOT IN (...)`
/// variant survives.
///
/// This is also the acceptance test for F-VAL-066 remediation option 3: after
/// that fix, the assertions must be inverted.
#[tokio::test]
async fn an_empty_retention_set_wipes_the_whole_table() {
    let store = store().await;

    for group in [GROUP, OTHER_GROUP] {
        store
            .store_keygen_secrets(
                group,
                ME,
                keygen::setup(&mut rand::thread_rng(), ME, COUNT, THRESHOLD).unwrap(),
            )
            .await
            .unwrap();
    }

    store.retain_keygen_secrets([]).await.unwrap();

    for group in [GROUP, OTHER_GROUP] {
        // A fresh insert now succeeds, which is only possible if the row is gone.
        let fresh = keygen::setup(&mut rand::thread_rng(), ME, COUNT, THRESHOLD).unwrap();
        let stored = store
            .store_keygen_secrets(group, ME, fresh.clone())
            .await
            .unwrap();
        assert_eq!(
            stored.commitment(),
            fresh.commitment(),
            "group {group} was wiped by the unqualified DELETE"
        );
    }
}

/// **F-VAL-066's nonces half (claim (b)).** A group first tracked by a *log*
/// transition has its freshly registered `nonces_chunks` row inside the delete's
/// scope, and the `ON DELETE CASCADE` takes all 1024 nonces with it
/// (`store.rs:80-87`). The `preprocess` commitment is already going onchain, so
/// the validator is committed to a Merkle root whose nonces it no longer holds —
/// the same signing blackout as F-VAL-030, from a different cause.
///
/// PASS ⇒ after the reconciliation, `nonces_reveal` and `take_nonce` both return
/// `None` for a chunk whose root is published onchain. Neither reviewer traced
/// this to that consequence; it should be picked up in remediation.
///
/// FAIL ⇒ the cascade does not fire (check that `PRAGMA foreign_keys` is on —
/// see §Known mechanical gaps in the README; if it is off, the `nonces` rows are
/// *orphaned* rather than deleted, which is a different and also reportable
/// defect: they then survive `retain_nonces` forever and F-VAL-035's
/// unpruned-secret concern grows).
#[tokio::test]
async fn reconciliation_cascades_away_a_committed_nonce_chunk() {
    let store = store().await;

    let chunk = NonceChunk::with_size(8, &KeyShare::dummy(), &mut rand::thread_rng()).unwrap();
    let root = store.register_nonces_chunk(GROUP, ME, chunk).await.unwrap();
    assert!(
        store.nonces_reveal(root, 0).await.unwrap().is_some(),
        "the chunk is usable before reconciliation"
    );

    // The block's `NewBlock` transition ran before its logs, so the retention
    // set does not name `GROUP`.
    store.retain_nonces([OTHER_GROUP]).await.unwrap();

    assert!(
        store.nonces_reveal(root, 0).await.unwrap().is_none(),
        "F-VAL-066(b): the committed nonce chunk is gone, root {root} is unsignable"
    );
    assert!(
        store.take_nonce(root, 0).await.unwrap().is_none(),
        "and every offset in it is gone with it"
    );
}
