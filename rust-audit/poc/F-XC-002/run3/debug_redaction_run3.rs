// rv-xc run 3, canonical row F-XC-002 (union of F-VAL-062 and F-CORE-036). Adapted from
// poc/F-VAL-062/debug_redaction.rs (V-VAL, run 1) to the effect and resume types at 361aaf3:
// `Effect::StartNonceGeneration` and the key-share-carrying `ReconcileGroupSecrets` are gone;
// `Effect::GenerateNonces` now carries the `Arc<KeyShare>` and `Resume::Nonces` the signing
// nonces. Appended to the end of `crates/validator/src/service/effect.rs`, reverted afterwards.
// Run: cargo test --offline --locked -p validator --bins service::effect::rv_xc_debug_redaction -- --nocapture
//
// The signing share of `KeyShare::dummy()` is `Scalar::ONE` and its identifier is also 1, so
// the check isolates the `signing_share` field before searching it (V-VAL's false-positive trap).

#[cfg(test)]
mod rv_xc_debug_redaction {
    use super::*;
    use alloy::primitives::address;

    const ONE_BE_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    /// The rendering of the `signing_share` field and whatever follows it up to the next comma.
    fn signing_share_field(rendered: &str) -> &str {
        let start = rendered.find("signing_share").expect("a KeyPackage rendering has signing_share");
        let rest = &rendered[start..];
        &rest[..rest.find(',').unwrap_or(rest.len())]
    }

    #[test]
    fn generate_nonces_effect_prints_the_key_share_with_its_signing_share_redacted() {
        // The `warn!(?effect, %err, "failed to perform effect")` sink (effect.rs:177) prints
        // this whole value if storing the nonces fails.
        let effect = Effect::GenerateNonces {
            group_id: B256::repeat_byte(0xa1),
            signature_id: B256::repeat_byte(0x51),
            key_share: Arc::new(KeyShare::dummy()),
        };
        let rendered = format!("{effect:?}");
        println!("Effect::GenerateNonces Debug: {rendered}");
        let field = signing_share_field(&rendered);
        println!("isolated: {field}");
        assert!(field.contains("<redacted>"));
        assert!(!field.contains(ONE_BE_HEX));
    }

    #[test]
    fn setup_resume_prints_secrets_with_frost_and_local_redactions() {
        // Printed by the `trace!(?resume, …)` sinks in safenet-core (effects.rs:66, :90,
        // driver.rs:267).
        let secrets = crate::frost::keygen::setup(
            &mut rand::thread_rng(),
            address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            3,
            2,
        )
        .unwrap();
        let resume = Resume::Setup {
            group_id: B256::repeat_byte(0xa1),
            secrets: Box::new(secrets),
        };
        let rendered = format!("{resume:?}");
        println!("Resume::Setup Debug: {rendered}");
        assert!(rendered.contains("EncryptionKey(\"redacted\")"));
        assert!(rendered.contains("coefficients: \"<redacted>\""));
    }

    #[test]
    fn nonces_resume_uses_the_hand_written_redaction() {
        let resume = Resume::Nonces {
            message: B256::repeat_byte(0x11),
            nonces: Box::new(SigningNonces::generate(&KeyShare::dummy(), &mut rand::thread_rng())),
        };
        let rendered = format!("{resume:?}");
        println!("Resume::Nonces Debug: {rendered}");
        assert!(rendered.contains("SigningNonces(\"<redacted>\")"));
    }
}
