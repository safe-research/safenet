//! PoC for the effect-handler half of **F-VAL-061** (and F-VAL-030's trigger B):
//! a failed effect is silently converted to `Resume::Noop`, with no retry path.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::service` by adding to
//! `crates/validator/src/service/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-030-032-061/effect_failure.rs"]
//! mod poc_f_val_061;
//! ```
//!
//! It must be a child of `crate::service` because `effect::Handler` is not
//! re-exported (`crates/validator/src/service/mod.rs:6-9` exports only `Action`,
//! `Effect` and `Resume`).

use super::effect::Handler;
use crate::{
    secrets::SecretStore,
    service::{Effect, Resume},
};
use alloy::primitives::{Address, B256, address};
use safenet_core::effects::EffectHandler as _;
use sqlx::sqlite::SqlitePool;
use std::collections::BTreeMap;

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const GROUP: B256 = B256::repeat_byte(0xa1);

async fn handler() -> Handler {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let secrets = SecretStore::new(pool).await.unwrap();
    Handler::new(ME, secrets)
}

/// **F-VAL-061 / F-VAL-030 trigger B.** A `NonceTree` effect for a group with no
/// process-local generator stream fails, and the failure is returned as
/// `Resume::Noop` — the same value a successful no-op returns.
///
/// This is the state the validator is in on the first `NewBlock` after every
/// restart: `Handler::new` builds an empty `NonceGenerator`
/// (`service/effect.rs:116-122`), and the only thing that starts a stream is
/// `Effect::ReconcileGroupSecrets` (`:231-235`), which the `NewBlock` transition
/// emits *after* `Effect::NonceTree` (`state/mod.rs:468-469`) and which the
/// driver spawns as a separate concurrent task (`core/driver.rs:266-274`).
///
/// PASS ⇒ `Resume::Noop`. The state machine cannot tell this from success, and
/// the `chunks[N] = None` reservation written by `handle_nonce_topup` before the
/// effect ran is now permanent (see `nonce_state.rs`). F-VAL-061's mechanism is
/// `E1`.
///
/// FAIL ⇒ the handler returns something other than `Resume::Noop` (or panics);
/// report it, because F-VAL-061's "exactly one failure policy" claim would be
/// wrong.
#[tokio::test]
async fn nonce_tree_without_a_generator_stream_resumes_as_noop() {
    let handler = handler().await;

    let resume = handler
        .perform_effect(Effect::NonceTree { group_id: GROUP })
        .await;

    assert!(
        matches!(resume, Resume::Noop),
        "expected Resume::Noop, got {resume:?}"
    );
}

/// The ordering that makes the above reachable without any crash: reconciling
/// first *does* start the stream, so a `NonceTree` after it succeeds. This pins
/// the dependency F-VAL-061 names and is the acceptance test for its
/// remediation option 4.
///
/// PASS ⇒ the same effect that just returned `Resume::Noop` returns
/// `Resume::NonceTree { .. }` once `ReconcileGroupSecrets` has run with the
/// group's key share. The two effects are emitted in the opposite order and
/// spawned concurrently, which is the bug.
///
/// NOTE: this test generates a full 1024-nonce chunk and will take several
/// seconds; that cost is itself part of F-VAL-038's argument.
#[tokio::test]
async fn reconciling_first_makes_the_same_effect_succeed() {
    use crate::frost::keygen::KeyShare;
    use std::sync::Arc;

    let handler = handler().await;

    let mut groups: BTreeMap<B256, Option<Arc<KeyShare>>> = BTreeMap::new();
    groups.insert(GROUP, Some(Arc::new(KeyShare::dummy())));
    let resume = handler
        .perform_effect(Effect::ReconcileGroupSecrets { groups })
        .await;
    assert!(matches!(resume, Resume::Noop));

    let resume = handler
        .perform_effect(Effect::NonceTree { group_id: GROUP })
        .await;
    assert!(
        matches!(resume, Resume::NonceTree { .. }),
        "with a stream started, the effect succeeds — the ordering is the bug, \
         not the effect. Got {resume:?}"
    );
}

/// Companion: `Effect::UseNonce` for coordinates that hold no nonce also
/// resumes as `Resume::Noop` (`service/effect.rs:189-201`).
///
/// This one is *correct* behaviour — it is the guard that makes
/// `take_nonce`'s deletion safe against replay — and it is included so a reader
/// can see that F-VAL-061's complaint is not "Noop is always wrong" but
/// "`Noop` is wrong for the two effects whose state is written *before* they
/// run".
#[tokio::test]
async fn use_nonce_on_a_missing_nonce_resumes_as_noop() {
    let handler = handler().await;

    let resume = handler
        .perform_effect(Effect::UseNonce {
            message: B256::repeat_byte(0x11),
            root: B256::repeat_byte(0x22),
            offset: 0,
        })
        .await;

    assert!(matches!(resume, Resume::Noop));
}
