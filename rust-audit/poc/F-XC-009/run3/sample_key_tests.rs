    // rv-xc run 3, F-XC-009: the shipped sample loads with the well-known key `1` as `signer` and
    // the live Gnosis RPC as `rpc`. Pasted at the end of `mod tests` in
    // `crates/validator/src/config.rs` and of `crates/sentinel/src/config.rs`, reverted with
    // `git checkout --` after the run.
    #[test]
    fn rv_xc_009_the_sample_loads_with_a_publicly_known_signer() {
        // `validator.sample.toml` or `sentinel.sample.toml`, next to the crate's manifest.
        const SAMPLE: &str = concat!(env!("CARGO_PKG_NAME"), ".sample.toml");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
        let config = toml::from_str::<Config>(&std::fs::read_to_string(path).unwrap()).unwrap();
        println!(
            "sample {SAMPLE}: rpc = {}, signer address = {}",
            config.rpc,
            config.signer.address()
        );
        // The address of private key 1, asserted by the crate's own test (validator config.rs:258).
        assert_eq!(
            config.signer.address(),
            alloy::primitives::address!("0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf")
        );
        assert_eq!(config.rpc.as_str(), "https://rpc.gnosischain.com/");
    }
