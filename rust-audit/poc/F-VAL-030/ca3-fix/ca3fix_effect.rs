//! CA3-FIX (2026-10-09): F-VAL-030's replacement (per-ceremony nonces) under
//! a restart or reorg that re-delivers the `Sign` and so re-emits
//! `GenerateNonces` for a ceremony this validator already served. Child module
//! of `crate::service::effect`, through the real `Handler` and `SecretStore`
//! (the store tests pin the SQL; this pins what the state machine resumes).
//!
//! Expected: a replay before use resumes with the first pair's commitments
//! (consistent with anything already committed onchain), a replay after use
//! resumes `Noop` (the validator sits that ceremony out), and a second use
//! resumes `Noop`. No fresh pair, no reuse, no persistent loss.

use super::{Effect, Handler, Resume};
use crate::{frost::keygen::KeyShare, secrets::SecretStore};
use alloy::primitives::{Address, B256};
use safenet_core::effects::EffectHandler as _;
use sqlx::SqlitePool;
use std::sync::Arc;

const ME: Address = Address::repeat_byte(0x11);

fn generate(key_share: &Arc<KeyShare>, sid: B256) -> Effect {
    Effect::GenerateNonces {
        group_id: B256::repeat_byte(0x42),
        signature_id: sid,
        key_share: key_share.clone(),
    }
}

#[tokio::test]
async fn w1_replayed_generate_nonces_never_yields_a_fresh_or_reused_pair() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let handler = Handler::new(ME, SecretStore::new(pool).await.unwrap());
    let ks = Arc::new(KeyShare::dummy());
    let sid = B256::repeat_byte(0x01);
    let message = B256::repeat_byte(0x71);

    let first = match handler.perform_effect(generate(&ks, sid)).await {
        Resume::NonceCommitments { signature_id, nonces } => {
            assert_eq!(signature_id, sid);
            nonces
        }
        other => panic!("first GenerateNonces: {other:?}"),
    };
    // Restart or reorg replays the `Sign`: the handler samples a fresh pair,
    // but the store keeps the first one.
    match handler.perform_effect(generate(&ks, sid)).await {
        Resume::NonceCommitments { nonces, .. } => {
            assert_eq!(nonces, first, "replay before use must resume with the first pair");
        }
        other => panic!("replayed GenerateNonces before use: {other:?}"),
    }
    println!("w1: replay before use resumes with the first pair's commitments");

    match handler.perform_effect(Effect::UseNonces { message, signature_id: sid }).await {
        Resume::Nonces { message: m, nonces } => {
            assert_eq!(m, message);
            assert_eq!(nonces.commitments(), first);
        }
        other => panic!("UseNonces: {other:?}"),
    }
    // A replay after use (e.g. a reorg that re-delivers the same signature id
    // for another message) must not hand out a fresh pair or the used one.
    let after = handler.perform_effect(generate(&ks, sid)).await;
    assert!(matches!(after, Resume::Noop), "replay after use resumed {after:?}");
    let reuse = handler.perform_effect(Effect::UseNonces { message: B256::repeat_byte(0x72), signature_id: sid }).await;
    assert!(matches!(reuse, Resume::Noop), "second use resumed {reuse:?}");
    println!("w1: replay after use resumes Noop for both GenerateNonces and UseNonces");

    // A different ceremony is unaffected: no phantom, no lost capacity.
    let next = handler.perform_effect(generate(&ks, B256::repeat_byte(0x02))).await;
    assert!(matches!(next, Resume::NonceCommitments { .. }), "next ceremony resumed {next:?}");
    println!("w1: the next ceremony gets its own pair");
}
