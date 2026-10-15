
    // ---------------------------------------------------------------------
    // QA2-CORE PoC for F2-CORE-030 (temporary edit; reverted after the run).
    // ---------------------------------------------------------------------

    /// F2-CORE-030, reorg variant. An event in block 1 emits effect 10; its
    /// resume is applied before block 2 is committed. Uncling block 2 restores
    /// snapshot 1 (which predates the resume) and returns no command, and the
    /// replay of block 2 emits nothing that would re-issue effect 10.
    #[tokio::test]
    async fn qa_f2_core_030_uncle_discards_applied_resume_and_never_reissues_effect() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(new_block(1)).await.unwrap();
        let commands = machine.handle_update(logs(1..=1, [10])).await.unwrap();
        println!("block 1 logs -> {commands:?}");
        assert!(commands.contains(&Command::Effect(10)));

        // The effect completes and its resume is applied to the live state.
        let commands = machine.handle_resume(777).await.unwrap();
        println!("resume 777 -> {commands:?}");
        assert_eq!(committed(&pool).await.unwrap().1.resumes, Vec::<u64>::new());

        // Block 2 commits; its snapshot is the first to carry the resume.
        machine.handle_update(new_block(2)).await.unwrap();
        machine.handle_update(logs(2..=2, [])).await.unwrap();
        let (block, state) = committed(&pool).await.unwrap();
        println!("committed after block 2: block {block}, {state:?}");
        assert_eq!(state.resumes, vec![777]);

        // Block 2 is uncled: the rollback to snapshot 1 emits NO commands.
        let commands = machine.handle_update(uncle(2)).await.unwrap();
        println!("Uncle{{2}} -> {commands:?}");
        assert_eq!(commands, vec![]);

        // The canonical block 2 is applied: block 1's event is not replayed, so
        // no effect is re-issued, and the new snapshot no longer has the resume.
        let c1 = machine.handle_update(new_block(2)).await.unwrap();
        let c2 = machine.handle_update(logs(2..=2, [])).await.unwrap();
        println!("replayed block 2 -> {c1:?} {c2:?}");
        assert!(
            !c1.iter().chain(c2.iter()).any(|c| matches!(c, Command::Effect(_))),
            "no effect was re-issued after the rollback"
        );
        let (block, state) = committed(&pool).await.unwrap();
        println!("committed after rollback + replay: block {block}, {state:?}");
        assert_eq!(state.events, vec![10], "the emitting event is still in the state");
        assert_eq!(state.resumes, Vec::<u64>::new(), "the applied resume 777 is gone");
    }

    /// F2-CORE-030, restart variant, pure state machine. Blocks 1..=6 are
    /// processed with driver-style pruning so the store ends holding {5, 6}
    /// (`safe` = 5). The event in block 5 (the anchor) emits effect 50 and its
    /// resume 555 is applied before block 6 commits. On restart the block
    /// watcher emits `Uncle{safe + 1}` = `Uncle{6}` (blocks.rs:261-266) and
    /// replays block 6 only; the restored state has no resume and nothing
    /// re-issues effect 50.
    #[tokio::test]
    async fn qa_f2_core_030_restart_discards_resume_of_effect_from_anchor_block() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        for block in 1..=6 {
            machine.handle_update(new_block(block)).await.unwrap();
            let events: Vec<u64> = if block == 5 { vec![50] } else { vec![] };
            let commands = machine
                .handle_update(logs(block..=block, events))
                .await
                .unwrap();
            if block == 5 {
                assert!(commands.contains(&Command::Effect(50)));
                // The effect resumes between the commits of blocks 5 and 6.
                machine.handle_resume(555).await.unwrap();
            }
            // The driver prunes with the watcher's `safe` (latest - 1 here).
            machine.prune(block.saturating_sub(1)).await.unwrap();
        }
        assert_eq!(
            machine.block_status().await.unwrap(),
            Some(BlockStatus { latest: 6, safe: 5 })
        );
        let (block, state) = committed(&pool).await.unwrap();
        println!("at shutdown: tip snapshot {block} = {state:?}");
        assert_eq!(state.resumes, vec![555]);
        drop(machine);

        // Restart: the watcher's first updates are Uncle{6}, then block 6 again.
        let mut machine = new_machine(&pool).await;
        let commands = machine.handle_update(uncle(6)).await.unwrap();
        println!("restart Uncle{{6}} -> {commands:?}");
        assert_eq!(commands, vec![]);
        let c1 = machine.handle_update(new_block(6)).await.unwrap();
        let c2 = machine.handle_update(logs(6..=6, [])).await.unwrap();
        println!("replayed block 6 -> {c1:?} {c2:?}");
        assert!(!c1.iter().chain(c2.iter()).any(|c| matches!(c, Command::Effect(_))));
        let (block, state) = committed(&pool).await.unwrap();
        println!("after restart replay: tip snapshot {block} = {state:?}");
        assert_eq!(state.events, vec![50]);
        assert_eq!(
            state.resumes,
            Vec::<u64>::new(),
            "resume 555 lost; effect 50 never re-issued"
        );
    }

    /// F2-CORE-030, Critic extension (replay ordering). When the emitting block
    /// IS replayed, the effect is re-emitted, but the driver spawns it only
    /// after `handle_update` returns (driver.rs:261 vs 278) while the next
    /// replayed block is already queued; the replayed follow-up event of block
    /// k+1 therefore reaches the transition before the re-spawned effect can
    /// resume.
    #[tokio::test]
    async fn qa_f2_core_030_replayed_followup_event_is_applied_before_the_respawned_effect_resumes() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        // First run: block 1 empty (becomes the anchor), proposal (event 10) at
        // block 2 -> effect 10, resume 777 applied, follow-up (event 11) at
        // block 3. Pruning leaves {1, 2, 3} with safe = 1.
        machine.handle_update(new_block(1)).await.unwrap();
        machine.handle_update(logs(1..=1, [])).await.unwrap();
        machine.handle_update(new_block(2)).await.unwrap();
        machine.handle_update(logs(2..=2, [10])).await.unwrap();
        machine.handle_resume(777).await.unwrap();
        machine.handle_update(new_block(3)).await.unwrap();
        machine.handle_update(logs(3..=3, [11])).await.unwrap();
        machine.prune(1).await.unwrap();
        assert_eq!(
            machine.block_status().await.unwrap(),
            Some(BlockStatus { latest: 3, safe: 1 })
        );
        drop(machine);

        // Restart: Uncle{2}, then blocks 2 and 3 are replayed back to back.
        let mut machine = new_machine(&pool).await;
        machine.handle_update(uncle(2)).await.unwrap();
        machine.handle_update(new_block(2)).await.unwrap();
        let commands = machine.handle_update(logs(2..=2, [10])).await.unwrap();
        println!("replayed block 2 -> {commands:?} (the driver spawns the effect only now)");
        assert!(commands.contains(&Command::Effect(10)));
        // The replayed block 3 is already queued by the watcher and is applied
        // before the re-spawned effect can resume.
        machine.handle_update(new_block(3)).await.unwrap();
        machine.handle_update(logs(3..=3, [11])).await.unwrap();
        let (block, state) = committed(&pool).await.unwrap();
        println!("replayed block 3 committed at {block}: {state:?}");
        assert_eq!(state.events, vec![10, 11]);
        assert_eq!(
            state.resumes,
            Vec::<u64>::new(),
            "follow-up event 11 applied with no resume in the state"
        );
        // Only afterwards does the resume land.
        machine.handle_resume(777).await.unwrap();
    }
