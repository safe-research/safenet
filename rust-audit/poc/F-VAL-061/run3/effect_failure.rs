//! RUN 3 (rv-val, 2026-10-08): F-VAL-061 and F-VAL-062 at `361aaf3`. Child
//! module of `crate::service::effect`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "<abs>/rust-audit/poc/F-VAL-061/run3/effect_failure.rs"]
//! mod poc_f_val_061_run3;
//! ```
//!
//! F-VAL-061: the real `Handler` over a secret store whose pool is closed, so
//! every effect fails inside `try_perform_effect`. Each resumes as
//! `Resume::Noop`, the same value an effect with nothing to report returns.
//!
//! F-VAL-062: what `tracing::warn!(?effect, ...)` prints for the one effect
//! that still carries a key share (`GenerateNonces`) and for the secret-bearing
//! resumes, with a real key share from a 2-of-3 DKG.

use super::*;
use crate::secrets::SecretStore;
use alloy::primitives::address;

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");

fn key_shares() -> BTreeMap<Address, KeyShare> {
    let mut rng = rand::thread_rng();
    let people = [ME, PEER1, PEER2];
    let mut secrets = BTreeMap::new();
    let mut verified = BTreeMap::new();
    for who in people {
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
    let mut out = BTreeMap::new();
    for (who, state) in states {
        let verified_shares = shares
            .iter()
            .map(|(peer, share)| {
                let enc = frost::keygen::verify_secret_share(state.group_commitments(), *peer, share).unwrap();
                (*peer, frost::keygen::verify_encrypted_secret_share(&state, *peer, enc).unwrap())
            })
            .collect();
        out.insert(who, frost::keygen::finalize(state, verified_shares).unwrap());
    }
    out
}

#[tokio::test]
async fn every_failed_effect_resumes_as_noop() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let store = SecretStore::new(pool.clone()).await.unwrap();
    let handler = Handler::new(ME, store);
    pool.close().await;

    let key_share = Arc::new(key_shares().remove(&ME).unwrap());
    let effects = vec![
        Effect::KeyGenSetup {
            group_id: B256::repeat_byte(1),
            count: 3,
            threshold: 2,
        },
        Effect::GenerateNonces {
            group_id: B256::repeat_byte(1),
            signature_id: B256::repeat_byte(2),
            key_share,
        },
        Effect::UseNonces {
            message: B256::repeat_byte(3),
            signature_id: B256::repeat_byte(2),
        },
        Effect::ReconcileGroupSecrets {
            block: 10,
            groups: BTreeMap::new(),
        },
    ];
    for effect in effects {
        let kind = match &effect {
            Effect::KeyGenSetup { .. } => "KeyGenSetup",
            Effect::GenerateNonces { .. } => "GenerateNonces",
            Effect::UseNonces { .. } => "UseNonces",
            Effect::ReconcileGroupSecrets { .. } => "ReconcileGroupSecrets",
        };
        let resume = handler.perform_effect(effect).await;
        println!("{kind}: closed pool -> {resume:?}");
        assert!(matches!(resume, Resume::Noop), "{kind} failure is a Noop");
    }
}

#[test]
fn debug_output_of_secret_bearing_effects_and_resumes() {
    let mut shares = key_shares();
    let key_share = shares.remove(&ME).unwrap();
    let json = serde_json::to_value(&key_share).unwrap();
    let scalar_hex = json["signing_share"].as_str().unwrap().to_owned();
    assert_eq!(scalar_hex.len(), 64, "signing share serialised as 32-byte hex");

    let effect = Effect::GenerateNonces {
        group_id: B256::repeat_byte(1),
        signature_id: B256::repeat_byte(2),
        key_share: Arc::new(key_share.clone()),
    };
    let printed = format!("{effect:?}");
    println!("Debug(GenerateNonces) = {} bytes; contains SigningShare(\"<redacted>\"): {}; contains the scalar hex: {}",
        printed.len(),
        printed.contains("SigningShare(\"<redacted>\")"),
        printed.contains(&scalar_hex));
    assert!(printed.contains("KeyShare"), "the key share is part of the printed effect");
    assert!(!printed.contains(&scalar_hex), "frost-core redacts the signing share");

    let secrets = frost::keygen::setup(&mut rand::thread_rng(), ME, 3, 2).unwrap();
    let resume = Resume::Setup {
        group_id: B256::repeat_byte(1),
        secrets: Box::new(secrets),
    };
    let printed = format!("{resume:?}");
    println!("Debug(Resume::Setup) redacts coefficients: {}; EncryptionKey redacted: {}",
        printed.contains("\"coefficients\": \"<redacted>\"") || printed.contains("coefficients: \"<redacted>\""),
        printed.contains("EncryptionKey(\"redacted\")"));
    assert!(printed.contains("EncryptionKey(\"redacted\")"));

    let nonces = frost::sign::SigningNonces::generate(&key_share, &mut rand::thread_rng());
    let printed = format!("{:?}", Resume::Nonces { message: B256::ZERO, nonces: Box::new(nonces) });
    println!("Debug(Resume::Nonces) = {printed}");
    assert!(printed.contains("<redacted>"));
}
