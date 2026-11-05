
    // ===================== LT3 lead 13 (F3-SEN-060) PoC =====================
    //
    // #974 drops the `WaitingForDisputeResolution` entry the moment it queues
    // `timeoutArbitration()` + `claim()`. If both of those revert onchain while
    // the request stays FROZEN (e.g. USDC.e has blacklisted the sponsor, so the
    // fee push in `timeoutArbitration` reverts and `claim` then reverts
    // `RequestNotResolved`), the entry is already gone, so the later
    // `ArbitrationTimedOut`/`DisputeResolved` (once the block clears and someone
    // times the request out) is ignored as untracked and the bond is never
    // claimed by the service.

    /// At `arbitration_deadline + 1` the dispute entry is dropped and the two
    /// recovery calls are queued; a subsequent terminal event for the same id
    /// finds nothing to claim.
    #[test]
    fn lead13_dropped_dispute_entry_leaves_no_claim_on_later_timeout() {
        let (svc, id, state) = setup_dispute();
        // deadline is 60 (see setup_dispute).

        // At the deadline block nothing is queued and the entry is retained.
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(60));
        assert!(commands.is_empty(), "no action at the deadline block");
        assert!(state.0.contains_key(&id), "entry retained at the deadline");

        // One block later the sentinel queues timeoutArbitration()+claim() and
        // DROPS the entry, regardless of whether those calls will succeed.
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(61));
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::TimeoutArbitration { id },
                    expires_at: None,
                }
                .into(),
                SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }
                .into(),
            ],
            "timeoutArbitration + claim are queued"
        );
        assert!(!state.0.contains_key(&id), "entry is dropped at deadline+1");

        // Onchain both of those revert (blacklisted sponsor: timeoutArbitration
        // reverts on the fee push, the request stays FROZEN, claim reverts
        // RequestNotResolved). The queue marks each mined revert executed by
        // nonce and never retries it. Nothing re-creates the entry.

        // Later, once the blacklist is lifted, someone times the request out.
        // The sentinel sees `ArbitrationTimedOut` but the entry is untracked,
        // so no `Claim` is emitted: the bond stays unclaimed by the service.
        let (state, commands) =
            svc.apply_transition(state, Message::Event(log(5000, arbitration_timed_out_event(id))));
        assert!(
            commands.is_empty(),
            "the recovery event produces no Claim: bond stranded until a manual claim"
        );
        assert!(!state.0.contains_key(&id));

        // Same outcome for a late DisputeResolved (if the arbitrator ruled after
        // the blacklist lifted): untracked, ignored, no Claim.
        let event = dispute_resolved_event(id, OnchainRequestState::RESOLVED_APPROVED, U256::ZERO);
        let (_state, commands) = svc.apply_transition(state, Message::Event(log(5001, event)));
        assert!(commands.is_empty(), "late DisputeResolved also ignored as untracked");
    }
