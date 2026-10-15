//! RUN 3 (rv-val, 2026-10-08): consult from the head (via rv-core-a, F-CORE-031).
//! What a repeated `GenerateNonces` / `UseNonces` does at the real `Handler`
//! when a rollback or restart replays the transition that emitted it. Child
//! module of `crate::service::effect`.

use super::*;
use crate::secrets::SecretStore;
use alloy::primitives::address;

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");

fn my_key_share() -> KeyShare {
    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut verified = BTreeMap::new();
    for who in [ME, PEER1, PEER2] {
        let s = frost::keygen::setup(&mut rng, who, 3, 2).unwrap();
        verified.insert(who, frost::keygen::verify_commitment(who, &s.commitment()).unwrap());
        secrets.insert(who, s);
    }
    let mut states = BTreeMap::new();
    let mut shares = BTreeMap::new();
    for (who, s) in secrets {
        let (state, share) = frost::keygen::generate_secret_shares(s, verified.clone()).unwrap();
        states.insert(who, state);
        shares.insert(who, share);
    }
    let state = states.remove(&ME).unwrap();
    let verified_shares = shares
        .iter()
        .map(|(peer, share)| {
            let enc = frost::keygen::verify_secret_share(state.group_commitments(), *peer, share).unwrap();
            (*peer, frost::keygen::verify_encrypted_secret_share(&state, *peer, enc).unwrap())
        })
        .collect();
    frost::keygen::finalize(state, verified_shares).unwrap()
}

#[tokio::test]
async fn replayed_effects_reuse_the_stored_pair_and_never_reuse_a_taken_one() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let handler = Handler::new(ME, SecretStore::new(pool).await.unwrap());
    let key_share = Arc::new(my_key_share());
    let (group_id, sid, message) = (B256::repeat_byte(0x42), B256::repeat_byte(0x01), B256::repeat_byte(0x77));
    let generate = || Effect::GenerateNonces {
        group_id,
        signature_id: sid,
        key_share: key_share.clone(),
    };

    let first = handler.perform_effect(generate()).await;
    let second = handler.perform_effect(generate()).await;
    let (a, b) = match (&first, &second) {
        (Resume::NonceCommitments { nonces: a, .. }, Resume::NonceCommitments { nonces: b, .. }) => (a.clone(), b.clone()),
        other => panic!("unexpected {other:?}"),
    };
    println!("GenerateNonces twice for one signature id: same commitments = {}", a == b);
    assert_eq!(a, b, "the second generation is discarded; the first stored pair is returned");

    let used = handler.perform_effect(Effect::UseNonces { message, signature_id: sid }).await;
    let reused = handler.perform_effect(Effect::UseNonces { message, signature_id: sid }).await;
    println!(
        "UseNonces twice: first = {}, second = {:?}",
        if matches!(used, Resume::Nonces { .. }) { "Nonces" } else { "other" },
        reused
    );
    assert!(matches!(used, Resume::Nonces { .. }));
    assert!(matches!(reused, Resume::Noop), "a taken pair is never handed out again");

    let regenerated = handler.perform_effect(generate()).await;
    println!("GenerateNonces after the pair was taken: {regenerated:?}");
    assert!(matches!(regenerated, Resume::Noop), "a used ceremony never gets a fresh pair");
}
