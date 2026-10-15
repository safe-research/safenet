
/// Q11 (rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) — what does
/// `estimate_eip1559_fees` issue and return when `reward` is empty?
///
/// Answered here with a mocked provider instead of anvil (no Foundry on the
/// host). The estimator is pure given (base_fee, rewards), so the mock is
/// equivalent: `alloy-provider-2.0.5/src/provider/trait.rs:276-306` issues one
/// `eth_feeHistory(0x0a, "latest", [20.0])` and passes
/// `fee_history.reward.unwrap_or_default()` to `eip1559_default_estimator`
/// (`alloy-provider-2.0.5/src/utils.rs:93-125`).
#[tokio::test]
async fn poc_q11_estimate_eip1559_fees_with_empty_reward() {
    let asserter = Asserter::new();
    let provider = Provider::mocked_with_chain(&asserter, CHAIN_ID);

    // Case A: `reward: Some(vec![])` — the shape a node with no non-zero tips
    // in the window returns.
    asserter.push_success(&FeeHistory {
        base_fee_per_gas: vec![100, 100],
        reward: Some(vec![]),
        ..Default::default()
    });
    let a = provider.estimate_eip1559_fees().await.unwrap();
    println!("reward = Some([])       -> {a:?}");

    // Case B: `reward: None` — the shape a node returns when no percentile was
    // honoured at all.
    asserter.push_success(&FeeHistory {
        base_fee_per_gas: vec![100, 100],
        reward: None,
        ..Default::default()
    });
    let b = provider.estimate_eip1559_fees().await.unwrap();
    println!("reward = None           -> {b:?}");

    // Case C: rewards present but all zero — filtered out by
    // `estimate_priority_fee`'s `filter(|r| **r > 0)`.
    asserter.push_success(&FeeHistory {
        base_fee_per_gas: vec![100, 100],
        reward: Some(vec![vec![0], vec![0]]),
        ..Default::default()
    });
    let c = provider.estimate_eip1559_fees().await.unwrap();
    println!("reward = [[0],[0]]      -> {c:?}");

    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(
        a.max_priority_fee_per_gas, 1,
        "EIP1559_MIN_PRIORITY_FEE (alloy-provider-2.0.5/src/utils.rs:26)"
    );
    assert_eq!(
        a.max_fee_per_gas, 201,
        "base_fee(100) * EIP1559_BASE_FEE_MULTIPLIER(2) + priority(1)"
    );
    assert!(asserter.read_q().is_empty(), "exactly one RPC call per estimate");
}
