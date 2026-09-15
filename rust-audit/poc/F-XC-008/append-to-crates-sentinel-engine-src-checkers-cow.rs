// PoC for F-XC-008 item 1 (also F-ENG-005, F-ENG-043, and questions 8 and 10 of
// rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md).
//
// NOT COMPILED, NOT RUN — no Rust toolchain on the audit machine.
//
// Append inside the existing `mod tests` block at the end of
// `crates/sentinel-engine/src/checkers/cow.rs` (before its closing brace).
// `ReqwestOrderApi` is private to that module, which is why the test has to
// live there — and why it exercises the real production type rather than a
// synthetic `reqwest` call.
//
// Run:  cargo test -p sentinel-engine qa_xc_008 -- --ignored --nocapture

#[tokio::test]
#[ignore = "spends 5 s of wall clock waiting for a request that never returns"]
async fn qa_xc_008_the_production_order_api_has_no_timeout() {
    // A server that completes the TCP handshake and then says nothing, ever.
    // This is the shape of the ordinary third-party availability event the
    // finding names: `api.cow.fi` accepting connections while its backend is
    // wedged. It is NOT a network partition (which the OS would eventually
    // reset) and needs no unusual failure mode.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            // Hold the socket so it is neither closed nor answered.
            held.push(socket);
        }
    });

    // Verbatim the production construction at
    // crates/sentinel-engine/src/checkers/cow.rs:228-231:
    //     pub fn new() -> Self { Self::with_client(reqwest::Client::new()) }
    let api = ReqwestOrderApi {
        client: reqwest::Client::new(),
    };

    let order_uid = Bytes::from(vec![0xab_u8; 56]);
    let base_url = format!("http://{addr}");

    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        api.fetch_order(&base_url, &order_uid),
    )
    .await;

    assert!(
        outcome.is_err(),
        "the default reqwest client returned within 5 s ({outcome:?}); it has an \
         internal cap after all, and F-XC-008 item 1 / F-ENG-005 / F-ENG-043 \
         should be re-scored downwards"
    );
}

#[tokio::test]
#[ignore = "spends 5 s of wall clock; the control for the test above"]
async fn qa_xc_008_a_configured_timeout_bounds_the_same_call() {
    // The control, and the remediation in one line: the same wedged server,
    // the same code path, a client built with a budget. If this one also
    // hangs, the problem is not the missing `.timeout(..)` and the proposed
    // fix is wrong.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });

    let api = ReqwestOrderApi {
        client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .connect_timeout(std::time::Duration::from_millis(500))
            .build()
            .unwrap(),
    };

    let order_uid = Bytes::from(vec![0xab_u8; 56]);
    let base_url = format!("http://{addr}");

    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        api.fetch_order(&base_url, &order_uid),
    )
    .await;

    assert!(
        outcome.is_ok(),
        "a 500 ms client timeout did not bound the call; the remediation for \
         F-XC-008 item 1 does not work as written"
    );
    assert!(
        outcome.unwrap().is_err(),
        "the bounded call must surface an error, which CowChecker turns into \
         Verdict::Abstain rather than a wrong verdict"
    );
}
