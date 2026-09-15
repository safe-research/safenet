//! PoC for F-VAL-062 — secret-bearing effects and resumes derive `Debug` and
//! are printed at `warn`.
//!
//! **This is the cheapest `E1` in the whole audit.** Critic C-VAL-B established
//! that `KeyShare::dummy()` (`crates/validator/src/frost/keygen.rs:443-453`)
//! has a *known* signing share — `k256::Scalar::ONE` — so a one-line
//! `format!("{:?}", …)` decides the `I`-class half of the finding without
//! reading `frost-core`'s source. Run this first.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::service` by adding to
//! `crates/validator/src/service/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-062/debug_redaction.rs"]
//! mod poc_f_val_062;
//! ```
//!
//! (`crate::service` is chosen because that is where the `warn!(?effect, ..)`
//! call site lives; the types are all reachable from anywhere in the crate.)

use crate::{
    frost::keygen::{self, KeyShare},
    service::{Effect, Resume},
};
use alloy::primitives::{Address, B256, address};
use std::{collections::BTreeMap, sync::Arc};

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const GROUP: B256 = B256::repeat_byte(0xa1);

/// The big-endian hex of `k256::Scalar::ONE`, which is the signing share inside
/// `KeyShare::dummy()` (`crates/validator/src/frost/keygen.rs:446-452`).
const ONE_BE_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000001";

/// Substrings that would betray the dummy signing share under the encodings a
/// derived `Debug` plausibly uses: full-width big-endian hex, the same with a
/// `0x` prefix, and the little-endian limb form `k256` uses internally.
fn leaky_substrings() -> Vec<String> {
    vec![
        ONE_BE_HEX.to_string(),
        ONE_BE_HEX.to_uppercase(),
        format!("0x{ONE_BE_HEX}"),
        // k256's internal representation is four 64-bit limbs; a derived Debug
        // over them would print the Montgomery form, not `1`, so the limb form
        // is checked only as a hint. Its absence proves nothing on its own —
        // read the printed output.
        "[1, 0, 0, 0]".to_string(),
    ]
}

fn assert_redacted(what: &str, rendered: &str) {
    println!("\n=== {what} ===\n{rendered}\n");
    for needle in leaky_substrings() {
        assert!(
            !rendered.contains(&needle),
            "F-VAL-062 CONFIRMED: the Debug rendering of {what} contains {needle:?}, \
             i.e. the secret scalar. This value is printed at `warn` by \
             `crates/validator/src/service/effect.rs:249`. Full rendering:\n{rendered}"
        );
    }
}

/// **The decisive test.** `Effect::StartNonceGeneration` carries an
/// `Arc<KeyShare>` and is printed whole by
/// `tracing::warn!(?effect, %err, "failed to perform effect")`
/// (`crates/validator/src/service/effect.rs:249`).
///
/// PASS (no assertion fires) ⇒ `frost-core 3.0.0` redacts `SigningShare` in its
/// `Debug`, so the `I`-class half of F-VAL-062 is **not reproduced**: no secret
/// reaches the log today. The finding stands only as the hardening item its
/// remediation option 1 describes (do not depend on an upstream implementation
/// detail), and its severity should be reconsidered — Low, not Medium.
///
/// FAIL ⇒ the secret scalar is in the log line. F-VAL-062 is confirmed at `E1`,
/// its severity is at least Medium and arguably High (`ReconcileGroupSecrets` is
/// emitted on *every block* and carries every tracked epoch's key share, and
/// `warn` is enabled by the shipped default filter), and remediation option 1
/// becomes urgent rather than prophylactic.
///
/// **Either way this is a real result — record it.** `--nocapture` prints the
/// rendering, which is the primary evidence; the assertions are a convenience.
#[test]
fn effect_debug_does_not_leak_the_signing_share() {
    let effect = Effect::StartNonceGeneration {
        group_id: GROUP,
        key_share: Arc::new(KeyShare::dummy()),
    };
    assert_redacted("Effect::StartNonceGeneration", &format!("{effect:?}"));
}

/// The same for `Effect::ReconcileGroupSecrets`, which is the reachable one:
/// `handle_group_reconciliation` emits it on every `NewBlock`
/// (`state/mod.rs:469`) carrying the key share of every tracked epoch, and its
/// handler performs two `DELETE`s on the shared pool — an `SQLITE_BUSY` there
/// prints the lot.
#[test]
fn reconcile_effect_debug_does_not_leak_the_signing_share() {
    let mut groups: BTreeMap<B256, Option<Arc<KeyShare>>> = BTreeMap::new();
    groups.insert(GROUP, Some(Arc::new(KeyShare::dummy())));
    groups.insert(B256::repeat_byte(0xb2), None);
    let effect = Effect::ReconcileGroupSecrets { groups };
    assert_redacted("Effect::ReconcileGroupSecrets", &format!("{effect:?}"));
}

/// `Resume::Setup` carries `Box<Secrets>`: the DKG polynomial coefficients and
/// the ECDH private key. `safenet-core` prints finished resumes at `trace`
/// (`crates/core/src/driver.rs:261`), which is what an operator turns on to
/// debug a stuck ceremony.
///
/// There is no known-value fixture for `Secrets` — `keygen::setup` samples
/// randomly and the fields are private — so this test cannot assert on a
/// specific scalar. Instead it checks the two things it *can*:
///
/// * the crate's own `EncryptionKey` redaction is present in the rendering
///   (`frost/ecdh.rs:50-54` prints `EncryptionKey("redacted")`), which proves
///   the rendering really is the derived one and the test is looking at the
///   right value;
/// * the rendering is **printed** for a human to read.
///
/// PASS ⇒ read the output. If `round1::SecretPackage`'s `Debug` shows
/// coefficients, that is the leak, and no assertion here can spot it
/// automatically. Compare the rendering against a second `Secrets` from a second
/// `setup` call: fields that differ between two runs are sampled secrets.
#[test]
fn print_the_resume_setup_debug_rendering_for_inspection() {
    let secrets = keygen::setup(&mut rand::thread_rng(), ME, 3, 2).unwrap();
    let other = keygen::setup(&mut rand::thread_rng(), ME, 3, 2).unwrap();

    let resume = Resume::Setup {
        group_id: GROUP,
        secrets: Box::new(secrets),
    };
    let rendered = format!("{resume:?}");
    let other = format!(
        "{:?}",
        Resume::Setup {
            group_id: GROUP,
            secrets: Box::new(other),
        }
    );

    println!("\n=== Resume::Setup (run 1) ===\n{rendered}\n");
    println!("=== Resume::Setup (run 2) ===\n{other}\n");
    println!(
        "Any substring that differs between the two runs is freshly sampled \
         material. If the two renderings are identical apart from the \
         proof-of-knowledge, nothing secret is printed."
    );

    assert!(
        rendered.contains("redacted"),
        "expected the crate's own EncryptionKey redaction (frost/ecdh.rs:50-54) \
         in the rendering; if it is missing, this test is not looking at the \
         derived Debug it thinks it is"
    );
}

/// Control: the crate's *hand-written* redactions do work. `Nonces` and
/// `NonceChunk` both have manual `Debug` impls
/// (`frost/preprocess.rs:64-71` and `:150-160`), and `Resume::Nonce` carries a
/// `Box<Nonces>`.
///
/// PASS ⇒ `Resume::Nonce`'s rendering contains `<redacted>` and not the nonce
/// scalars. This is the standard F-VAL-062 says `KeyShare` and `Secrets` should
/// be held to, and it shows remediation option 1 is a five-line change with a
/// working precedent in the same crate.
#[test]
fn hand_written_redactions_work() {
    let mut chunk =
        crate::frost::preprocess::NonceChunk::with_size(1, &KeyShare::dummy(), &mut rand::thread_rng())
            .unwrap();
    let nonces = chunk.nonces.remove(0);
    let resume = Resume::Nonce {
        message: B256::repeat_byte(0x11),
        nonces: Box::new(nonces),
    };
    let rendered = format!("{resume:?}");
    println!("\n=== Resume::Nonce ===\n{rendered}\n");
    assert!(
        rendered.contains("<redacted>"),
        "Nonces has a hand-written redacting Debug; expected it in the output"
    );
}
