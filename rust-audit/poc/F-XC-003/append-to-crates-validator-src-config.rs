// PoC for F-XC-003 (validator half) — question 3 of
// rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md.
//
// NOT COMPILED, NOT RUN — no Rust toolchain on the audit machine.
//
// Append inside the existing `mod tests` block at the end of
// `crates/validator/src/config.rs` (i.e. paste these three functions before its
// closing brace). They use `toml::from_str::<Config>` exactly as the four tests
// already there do.
//
// Run:  cargo test -p validator config::tests::qa_xc_003
//
// These are deliberately modelled on `sentinel-engine`'s existing
// `rejects_unknown_field` (crates/sentinel-engine/src/config.rs:129-144) — the
// only unknown-field test in the workspace, and the one config that has no
// `#[serde(flatten)]` field, so it proves nothing about the interaction.

#[test]
fn qa_xc_003_rejects_an_unknown_top_level_key() {
    // The container carries BOTH `#[serde(deny_unknown_fields)]`
    // (crates/validator/src/config.rs:21) and
    // `#[serde(default, flatten)] pub driver: driver::Config` (:36-38).
    //
    // `driver::Config` itself is `#[serde(default)]` with NO
    // `deny_unknown_fields` (crates/core/src/driver.rs:29-37), so if the outer
    // attribute is inert the stray key is routed into the flattened struct and
    // silently discarded.
    let result = toml::from_str::<Config>(
        r#"
            rpc = "https://eth.llamarpc.com"
            signer = "0x0000000000000000000000000000000000000000000000000000000000000001"
            database = "sqlite:validator.db"
            not_a_real_field = "typo"

            [validator]
            consensus = "0x0000000000000000000000000000000000000000"
        "#,
    );

    assert!(
        result.is_err(),
        "a mistyped top-level key was accepted in silence: {:?}",
        result.map(|c| c.rpc)
    );
}

#[test]
fn qa_xc_003_rejects_a_typo_in_a_security_relevant_key() {
    // The realistic instance, and the reason this is a finding rather than a
    // style note: `use_client_filtering` is the switch that makes the event
    // watcher verify `eth_getLogs` results client-side instead of trusting the
    // node (A4). `[index]` deserializes into `core::index::Config`, which has
    // `deny_unknown_fields` AND two `#[serde(flatten)]` fields
    // (crates/core/src/index/mod.rs:20-29) — the same combination, one level
    // down, and reached through the outer flatten.
    let result = toml::from_str::<Config>(
        r#"
            rpc = "https://eth.llamarpc.com"
            signer = "0x0000000000000000000000000000000000000000000000000000000000000001"
            database = "sqlite:validator.db"

            [validator]
            consensus = "0x0000000000000000000000000000000000000000"

            [index]
            use_client_filterring = true
        "#,
    );

    assert!(
        result.is_err(),
        "a typo inside [index] was accepted in silence; the validator would run \
         with node-filtered, unverified eth_getLogs results while the operator \
         believes client-side verification is on"
    );
}

#[test]
fn qa_xc_003_rejects_a_mistyped_table_name() {
    // A mistyped optional *table* is the cheapest real-world version: the
    // service runs on defaults with no startup echo of the resolved config
    // (crates/validator/src/main.rs:43 logs only the file path) and no metric.
    let result = toml::from_str::<Config>(
        r#"
            rpc = "https://eth.llamarpc.com"
            signer = "0x0000000000000000000000000000000000000000000000000000000000000001"
            database = "sqlite:validator.db"

            [validator]
            consensus = "0x0000000000000000000000000000000000000000"

            [observabilty]
            log_filter = "validator=debug,info"
        "#,
    );

    assert!(
        result.is_err(),
        "a mistyped [observabilty] table was accepted in silence"
    );
}
