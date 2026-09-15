// PoC for F-XC-003 (sentinel half) — question 3 of
// rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md.
//
// NOT COMPILED, NOT RUN — no Rust toolchain on the audit machine.
//
// Append inside the existing `mod tests` block at the end of
// `crates/sentinel/src/config.rs` (before its closing brace). It reuses that
// module's own `TOML` fixture constant (crates/sentinel/src/config.rs:74-84),
// the same way `rejects_config_missing_a_deployment_specific_field` does.
//
// Run:  cargo test -p sentinel config::tests::qa_xc_003
//
// The sentinel's `Config` is the sharper of the two cases: its flatten has no
// `default` — `#[serde(flatten)] pub driver: driver::Config`
// (crates/sentinel/src/config.rs:35-37) — while the validator's does
// (`#[serde(default, flatten)]`, crates/validator/src/config.rs:36-38). If the
// two behave differently, that difference is itself the answer to question 3.

#[test]
fn qa_xc_003_rejects_an_unknown_top_level_key() {
    // Inserted BEFORE the first table header, so it really is a top-level key
    // and not a member of `[sentinel]`.
    let with_typo = TOML.replacen(
        r#"rpc = "https://eth.llamarpc.com""#,
        "not_a_real_field = \"typo\"\n        rpc = \"https://eth.llamarpc.com\"",
        1,
    );

    let result = toml::from_str::<Config>(&with_typo);

    assert!(
        result.is_err(),
        "a mistyped top-level key was accepted in silence"
    );
}

#[test]
fn qa_xc_003_rejects_a_typo_inside_the_sentinel_table() {
    // `SentinelConfig` carries `deny_unknown_fields` and NO flattened field
    // (crates/sentinel/src/config.rs:48-58), so this one should pass whatever
    // serde does with the outer container. It is the control: if this fails,
    // the problem is the fixture, not the flatten interaction.
    let with_typo = TOML.replace("voting_window = 100", "votin_window = 100");

    let result = toml::from_str::<Config>(&with_typo);

    assert!(
        result.is_err(),
        "a typo inside [sentinel] was accepted; deny_unknown_fields is inert \
         even without a flattened field, which would be a much larger problem"
    );
}

#[test]
fn qa_xc_003_rejects_a_typo_in_a_flattened_driver_key() {
    // `[index] max_reorg_depth` reaches `core::index::blocks::Config` through
    // two levels of flatten. The validator's own
    // `deserializes_with_optional_service_fields` test proves the *positive*
    // path works (crates/validator/src/config.rs:235-275); nothing anywhere
    // proves the negative one.
    let with_typo = format!("{TOML}\n        [index]\n        max_reorg_dept = 12\n");

    let result = toml::from_str::<Config>(&with_typo);

    assert!(
        result.is_err(),
        "a typo inside [index] was accepted in silence; the sentinel would run \
         on the default max_reorg_depth (A5) while the operator believes it is 12"
    );
}
