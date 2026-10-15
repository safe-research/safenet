    // ======================================================================
    // QA2-XC proof-of-concept modules (run 2). Temporarily pasted at the end
    // of `crates/core/src/state/mod.rs`'s `mod tests` block and reverted with
    // `git checkout -- crates/core/src/state/mod.rs` after the run.
    //
    //   qa2_xc_050   F2-XC-050 (canonical over F2-CORE-036): snapshot committed
    //                before the page's actions are enqueued; with a single
    //                retained snapshot a fault in that window loses them.
    //   qa2_xc_006   F2-XC-006: a store written under one chain id is resumed
    //                under another; the queued request is broadcast there.
    //                Needs two local Anvil nodes (poc/F2-XC-006/run.sh).
    //   qa2_cov_7_2  coverage.md section 7 item 2: restart replay with a
    //                file-backed database against one local Anvil node
    //                (poc/F2-XC-050/coverage-7.2/run.sh).
    //
    // Shared helpers live in `qa2_xc_common`.
    // ======================================================================
    mod qa2_xc_common {
        use super::*;
        use crate::{
            index::blocks::{BlockTime, BlockWatcher, Config as BlockConfig},
            provider::Provider,
            tx::{Signer, Transaction},
            utils::connect_sqlite,
        };
        use alloy::{
            eips::BlockId,
            primitives::{B256, keccak256},
            providers::Provider as _,
        };
        use k256::ecdsa::SigningKey;
        use sqlx::sqlite::SqliteConnectOptions;
        use std::{str::FromStr, time::Duration};

        /// A file-backed pool opened the way the services open theirs
        /// (`utils::connect_sqlite`), so that a "restart" can close every
        /// connection and reopen the same file.
        pub async fn file_pool(path: &str) -> SqlitePool {
            let options = SqliteConnectOptions::from_str(&format!("sqlite:{path}?mode=rwc"))
                .unwrap()
                // sqlx's default is 5 s; shortened so an injected lock fails fast.
                .busy_timeout(Duration::from_millis(250));
            connect_sqlite(options).await.unwrap()
        }

        /// A fresh database path under `$QA2_XC_DIR` (stale files removed).
        pub fn fresh_db(name: &str) -> String {
            let dir = std::env::var("QA2_XC_DIR").expect("QA2_XC_DIR must name a scratch directory");
            let path = format!("{dir}/{name}.db");
            for suffix in ["", "-journal", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{path}{suffix}"));
            }
            path
        }

        pub fn env_or(name: &str, default: &str) -> String {
            std::env::var(name).unwrap_or_else(|_| default.to_owned())
        }

        /// Anvil's first default account (a public test key), funded on every
        /// local Anvil chain.
        const ANVIL_KEY_0: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

        pub fn anvil_signer() -> Signer {
            Signer::new(SigningKey::from_slice(&alloy::hex::decode(ANVIL_KEY_0).unwrap()).unwrap())
        }

        pub fn test_signer() -> Signer {
            Signer::new(SigningKey::from_slice(keccak256("qa2-xc").as_slice()).unwrap())
        }

        pub fn block_config(max_reorg_depth: u64) -> BlockConfig {
            BlockConfig {
                block_time: BlockTime::Millis(1_000),
                max_reorg_depth,
                ..Default::default()
            }
        }

        /// The driver's action encoding for the test transition: one
        /// never-expiring transaction per `Action::Event` (its number as
        /// calldata), as `Driver::update` does at `driver.rs:272-277`.
        pub fn encode(commands: &[Command<Action, u64>]) -> Vec<(Transaction, Option<u64>)> {
            commands
                .iter()
                .filter_map(|command| match command {
                    Command::Action(Action::Event(event)) => Some((
                        Transaction {
                            to: alloy::primitives::address!("000000000000000000000000000000000000c0de"),
                            data: [&[0x5a, 0xfe][..], &event.to_be_bytes()[..]].concat().into(),
                            gas: 50_000,
                            ..Default::default()
                        },
                        None,
                    )),
                    _ => None,
                })
                .collect()
        }

        /// Feeds one block update to the state machine the way `Watcher::next`
        /// plus `Driver::update` would: a `New`/`Warp` is followed by its log
        /// range (carrying `events`), an `Uncle` stands alone. Returns every
        /// command produced.
        pub async fn apply(
            machine: &mut StateMachine<TestState, TestTransition>,
            update: BlockUpdate,
            events: impl IntoIterator<Item = u64>,
        ) -> Vec<Command<Action, u64>> {
            let mut commands = machine.handle_update(Update::Block(update.clone())).await.unwrap();
            let range = match update {
                BlockUpdate::New { number, .. } => number..=number,
                BlockUpdate::Warp { from, to } => from..=to,
                BlockUpdate::Uncle { .. } => return commands,
            };
            commands.extend(machine.handle_update(logs(range, events)).await.unwrap());
            commands
        }

        pub async fn transaction_rows(pool: &SqlitePool) -> Vec<(Option<i64>, String, Option<i64>, Option<i64>)> {
            sqlx::query_as("SELECT nonce, request, submitted_at, executed_at FROM transactions ORDER BY id")
                .fetch_all(pool)
                .await
                .unwrap()
        }

        pub async fn block_hash(provider: &Provider, number: u64) -> B256 {
            provider
                .get_block(BlockId::number(number))
                .hashes()
                .await
                .unwrap()
                .unwrap_or_else(|| panic!("block {number} missing"))
                .header
                .hash
        }

        pub async fn anvil(provider: &Provider, method: &'static str, params: serde_json::Value) {
            let _: serde_json::Value = provider.raw_request(method.into(), params).await.unwrap();
        }

        /// A `BlockWatcher` over a mocked node at block 1000 (as in
        /// `blocks.rs`'s own tests), initialised from `indexed`; returns the
        /// updates `initialize` queued (`blocks.rs:244-289`).
        pub async fn mocked_restart(indexed: BlockStatus, max_reorg_depth: u64) -> (BlockWatcher, Vec<BlockUpdate>) {
            use alloy::{
                rpc::types::{Block, Header},
                transports::mock::Asserter,
            };
            fn hash(number: u64) -> B256 {
                keccak256(number.to_be_bytes())
            }
            fn block(number: u64) -> Block {
                Block::empty(Header {
                    hash: hash(number),
                    inner: alloy::consensus::Header {
                        parent_hash: number.checked_sub(1).map(hash).unwrap_or_default(),
                        number,
                        timestamp: 1_700_000_000 + number * 2,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            }
            let asserter = Asserter::new();
            asserter.push_success(&block(1000));
            for number in (1000 - max_reorg_depth)..1000 {
                asserter.push_success(&block(number));
            }
            let mut watcher = BlockWatcher::new(
                Provider::mocked(&asserter),
                BlockConfig {
                    block_time: BlockTime::Millis(2_000),
                    max_reorg_depth,
                    ..Default::default()
                },
                Some(indexed),
            )
            .await
            .unwrap();
            let updates = watcher.ready().collect::<Vec<_>>();
            assert!(asserter.read_q().is_empty());
            (watcher, updates)
        }
    }

    // ----------------------------------------------------------------------
    // F2-XC-050
    // ----------------------------------------------------------------------
    mod qa2_xc_050 {
        use super::{qa2_xc_common::*, *};
        use crate::{
            provider::Provider,
            tx::{self, Config as TxConfig, TransactionQueue},
        };
        use alloy::transports::mock::Asserter;
        use sqlx::{
            Connection,
            sqlite::{SqliteConnectOptions, SqliteConnection},
        };
        use std::str::FromStr;

        /// Runs the driver's sequence for one committed page whose commands are
        /// `commands` (`driver.rs:261-290`): `prune(safe)`, encode, enqueue --
        /// with the enqueue failing on a storage fault (another connection holds
        /// the write lock, so the `INSERT` at `tx/storage.rs:96` gets
        /// `SQLITE_BUSY` past `busy_timeout`). Asserts the driver would exit,
        /// then "kills" the process: drops everything and closes the pool.
        async fn prune_then_fail_enqueue(
            path: &str,
            pool: SqlitePool,
            machine: StateMachine<TestState, TestTransition>,
            safe: u64,
            commands: &[Command<Action, u64>],
        ) {
            let mut queue = TransactionQueue::new(
                Provider::mocked(&Asserter::new()),
                test_signer(),
                pool.clone(),
                TxConfig::default(),
            )
            .await
            .unwrap();

            // driver.rs:263
            machine.prune(safe).await.unwrap();
            // driver.rs:272-277
            let transactions = encode(commands);
            assert_eq!(transactions.len(), 1, "the page produced exactly one action");

            // The fault: driver.rs:283 cannot commit its own transaction.
            let mut blocker = SqliteConnection::connect_with(
                &SqliteConnectOptions::from_str(&format!("sqlite:{path}")).unwrap(),
            )
            .await
            .unwrap();
            sqlx::query("BEGIN IMMEDIATE").execute(&mut blocker).await.unwrap();
            let err = queue
                .queue(transactions)
                .await
                .expect_err("enqueue must fail while the write lock is held");
            println!("enqueue error at driver.rs:283: {err:?}");
            assert!(matches!(err, tx::Error::Storage(_)));
            // driver.rs:284: not intermittent, so `?` propagates it and
            // `Driver::run` logs "unrecoverable driver error; exiting".
            assert!(tx::lift_intermittent_error::<()>(Err(err)).is_err());

            // Process gone (the same picture as a SIGKILL/OOM between :263 and :283).
            drop(queue);
            drop(machine);
            pool.close().await;
            sqlx::query("ROLLBACK").execute(&mut blocker).await.unwrap();
            blocker.close().await.unwrap();
        }

        /// Restarts over `path`: `StateMachine::new`, `TransactionQueue::new`,
        /// and the real `BlockWatcher::initialize` for the persisted status.
        async fn restart(
            path: &str,
            max_reorg_depth: u64,
        ) -> (SqlitePool, StateMachine<TestState, TestTransition>, BlockStatus, Vec<BlockUpdate>) {
            let pool = file_pool(path).await;
            let machine = new_machine(&pool).await;
            let _queue = TransactionQueue::new(
                Provider::mocked(&Asserter::new()),
                test_signer(),
                pool.clone(),
                TxConfig::default(),
            )
            .await
            .unwrap();
            let status = machine.block_status().await.unwrap().unwrap();
            let (_watcher, updates) = mocked_restart(status, max_reorg_depth).await;
            println!("restart: persisted {status:?}; watcher queued {updates:?}");
            (pool, machine, status, updates)
        }

        /// (a) Catch-up warp page: the page's `prune(to)` leaves one row, the
        /// enqueue fails, the restart emits no `Uncle`, the action is gone.
        #[tokio::test]
        async fn warp_page_actions_are_lost_when_enqueue_fails_after_the_commit() {
            let path = fresh_db("xc050-warp-page");
            let pool = file_pool(&path).await;
            let mut machine = new_machine(&pool).await;

            // Catch-up warp 1..=6: the watcher's `safe` is the warp's `to` (blocks.rs:274-277).
            assert_eq!(machine.handle_update(warp(1, 6)).await.unwrap(), vec![]);
            // Page 1..=3 with one event: `handle_update` commits snapshot 3
            // (state/mod.rs:236) and returns the page's commands.
            let commands = machine.handle_update(logs(1..=3, [10])).await.unwrap();
            assert_eq!(commands, vec![Command::Action(Action::Event(10)), Command::Effect(10)]);

            prune_then_fail_enqueue(&path, pool, machine, 6, &commands).await;

            let (pool, mut machine, status, updates) = restart(&path, 2).await;
            assert_eq!(status, BlockStatus { latest: 3, safe: 3 });
            // No `Uncle` (blocks.rs:261-266): the page is never replayed.
            assert!(!updates.iter().any(|update| matches!(update, BlockUpdate::Uncle { .. })));
            assert_eq!(updates[0], BlockUpdate::Warp { from: 4, to: 998 });
            assert_eq!(apply(&mut machine, updates[0].clone(), []).await, vec![]);

            let (block, state) = committed(&pool).await.unwrap();
            let rows = transaction_rows(&pool).await;
            println!("after restart: snapshot {block} says events {:?} were handled; transactions rows = {rows:?}", state.events);
            assert_eq!(state.events, vec![10]); // the snapshot records event 10 as handled ...
            assert!(rows.is_empty()); // ... and the queue never received its action: lost.
        }

        /// (b) `max_reorg_depth = 0`: every block is pruned to a single row.
        #[tokio::test]
        async fn depth_zero_block_actions_are_lost_when_enqueue_fails_after_the_commit() {
            let path = fresh_db("xc050-depth-zero");
            let pool = file_pool(&path).await;
            let mut machine = new_machine(&pool).await;

            for block in 1..=2 {
                machine.handle_update(new_block(block)).await.unwrap();
                machine.handle_update(logs(block..=block, [])).await.unwrap();
                machine.prune(block).await.unwrap(); // depth 0: safe == latest (blocks.rs:453-461)
            }
            machine.handle_update(new_block(3)).await.unwrap();
            let commands = machine.handle_update(logs(3..=3, [30])).await.unwrap();
            assert_eq!(commands, vec![Command::Action(Action::Event(30)), Command::Effect(30)]);

            prune_then_fail_enqueue(&path, pool, machine, 3, &commands).await;

            let (pool, mut machine, status, updates) = restart(&path, 0).await;
            assert_eq!(status, BlockStatus { latest: 3, safe: 3 });
            assert!(!updates.iter().any(|update| matches!(update, BlockUpdate::Uncle { .. })));
            assert_eq!(updates[0], BlockUpdate::Warp { from: 4, to: 1000 });
            assert_eq!(apply(&mut machine, updates[0].clone(), []).await, vec![]);

            let (_, state) = committed(&pool).await.unwrap();
            let rows = transaction_rows(&pool).await;
            println!("after restart: state events {:?}; transactions rows = {rows:?}", state.events);
            assert_eq!(state.events, vec![30]);
            assert!(rows.is_empty());
        }

        /// Control: steady state with `max_reorg_depth = 2` keeps two rows, so
        /// the same fault at the same place is recovered by the restart
        /// `Uncle` replay (as a duplicate-prone re-emission, F2-CORE-032/063).
        #[tokio::test]
        async fn control_two_retained_snapshots_replay_the_page_and_recover_the_action() {
            let path = fresh_db("xc050-control");
            let pool = file_pool(&path).await;
            let mut machine = new_machine(&pool).await;

            for block in 1..=6 {
                machine.handle_update(new_block(block)).await.unwrap();
                machine.handle_update(logs(block..=block, [])).await.unwrap();
                machine.prune(block.saturating_sub(2)).await.unwrap();
            }
            machine.handle_update(new_block(7)).await.unwrap();
            let commands = machine.handle_update(logs(7..=7, [70])).await.unwrap();
            assert_eq!(commands, vec![Command::Action(Action::Event(70)), Command::Effect(70)]);

            prune_then_fail_enqueue(&path, pool, machine, 5, &commands).await;

            let (pool, mut machine, status, updates) = restart(&path, 2).await;
            assert_eq!(status, BlockStatus { latest: 7, safe: 5 });
            assert_eq!(updates[0], BlockUpdate::Uncle { number: 6 });
            assert_eq!(updates[1], BlockUpdate::Warp { from: 6, to: 998 });
            assert_eq!(apply(&mut machine, updates[0].clone(), []).await, vec![]);
            // The replayed range re-delivers block 7's log, so the action is
            // re-emitted and can be enqueued this time.
            let replayed = apply(&mut machine, updates[1].clone(), [70]).await;
            assert_eq!(replayed, vec![Command::Action(Action::Event(70)), Command::Effect(70)]);
            let mut queue = TransactionQueue::new(
                Provider::mocked(&Asserter::new()),
                test_signer(),
                pool.clone(),
                TxConfig::default(),
            )
            .await
            .unwrap();
            queue.queue(encode(&replayed)).await.unwrap();
            let rows = transaction_rows(&pool).await;
            println!("control after restart: transactions rows = {rows:?}");
            assert_eq!(rows.len(), 1);
        }
    }

    // ----------------------------------------------------------------------
    // F2-XC-006 (two local Anvil nodes: QA2_XC_RPC_A on one chain id,
    // QA2_XC_RPC_B on another; run through poc/F2-XC-006/run.sh)
    // ----------------------------------------------------------------------
    mod qa2_xc_006 {
        use super::{qa2_xc_common::*, *};
        use crate::{
            index::blocks::BlockWatcher,
            provider::Provider,
            tx::{Config as TxConfig, TransactionQueue},
        };
        use alloy::providers::Provider as _;
        use url::Url;

        #[tokio::test]
        async fn a_store_written_on_one_chain_is_resumed_on_another_and_its_queued_request_is_broadcast_there() {
            let path = fresh_db("xc006");
            let rpc_a = env_or("QA2_XC_RPC_A", "http://127.0.0.1:8745");
            let rpc_b = env_or("QA2_XC_RPC_B", "http://127.0.0.1:8746");
            let signer = anvil_signer();
            let signer_address = signer.address();

            // ---- run 1: `rpc` = chain A. Index the head, commit snapshots,
            // queue a block's action (not yet submitted when the process stops).
            let pool = file_pool(&path).await;
            let a = Provider::connect(&Url::parse(&rpc_a).unwrap()).await.unwrap();
            println!("run 1: rpc {rpc_a} -> chain_id {}", a.chain_id());
            let mut machine = new_machine(&pool).await;
            let mut watcher = BlockWatcher::new(a.clone(), block_config(2), None).await.unwrap();
            let updates = watcher.ready().collect::<Vec<_>>();
            let mut commands = vec![];
            for update in updates {
                let BlockUpdate::New { number, hash, .. } = update else { panic!("fresh start emits New only") };
                println!("run 1: indexed chain {} block {number} hash {hash}", a.chain_id());
                commands.extend(apply(&mut machine, update, [number]).await);
            }
            machine.prune(watcher.status().safe).await.unwrap();
            let status_a = machine.block_status().await.unwrap().unwrap();
            let (tip, state_a) = committed(&pool).await.unwrap();
            println!("run 1: store {status_a:?}; snapshot {tip} state {state_a:?}");
            let hash_a = block_hash(&a, status_a.latest).await;
            let mut queue = TransactionQueue::new(a.clone(), signer.clone(), pool.clone(), TxConfig::default())
                .await
                .unwrap();
            let queued = encode(&commands[commands.len() - 2..]); // the tip block's action only
            assert_eq!(queued.len(), 1);
            println!("run 1: queued request {:?}", queued[0].0);
            queue.queue(queued).await.unwrap(); // no block status yet: enqueued, not submitted
            assert_eq!(a.get_transaction_count(signer_address).await.unwrap(), 0);
            drop(queue);
            drop(watcher);
            drop(machine);
            pool.close().await;

            // ---- run 2: same `database`, `rpc` edited to chain B.
            let pool = file_pool(&path).await;
            let b = Provider::connect(&Url::parse(&rpc_b).unwrap()).await.unwrap();
            println!("run 2: rpc {rpc_b} -> chain_id {}", b.chain_id());
            assert_ne!(a.chain_id(), b.chain_id());
            let mut machine = new_machine(&pool).await; // resumes; nothing to compare against
            let status = machine.block_status().await.unwrap().unwrap();
            assert_eq!(status, status_a);
            let hash_b = block_hash(&b, status.latest).await;
            println!(
                "run 2: block {} hash on chain {} = {hash_a}, on chain {} = {hash_b}",
                status.latest,
                a.chain_id(),
                b.chain_id()
            );
            assert_ne!(hash_a, hash_b);
            // The watcher resumes by number on the other chain: no error, no warning.
            let mut watcher = BlockWatcher::new(b.clone(), block_config(2), Some(status)).await.unwrap();
            let updates = watcher.ready().collect::<Vec<_>>();
            println!("run 2: watcher queued {updates:?}");
            for update in updates {
                apply(&mut machine, update, []).await;
            }
            machine.prune(watcher.status().safe).await.unwrap();
            let (tip, state_b) = committed(&pool).await.unwrap();
            println!(
                "run 2: snapshot {tip} state {state_b:?} (events derived from chain {} blocks, now applied on chain {})",
                a.chain_id(),
                b.chain_id()
            );
            assert!(!state_b.events.is_empty());
            // The request queued while on chain A is signed with chain B's id
            // (tx/mod.rs:246-248) and broadcast there.
            let mut queue = TransactionQueue::new(b.clone(), signer, pool.clone(), TxConfig::default())
                .await
                .unwrap();
            let nonce_before = b.get_transaction_count(signer_address).await.unwrap();
            queue.update_block_status(watcher.status()).await.unwrap();
            let rows = transaction_rows(&pool).await;
            // Anvil automines asynchronously after `eth_sendRawTransaction`
            // returns, so poll for the inclusion rather than reading at once.
            let mut nonce = nonce_before;
            for _ in 0..50 {
                nonce = b.get_transaction_count(signer_address).await.unwrap();
                if nonce > nonce_before {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            println!(
                "run 2: transactions rows {rows:?}; signer nonce on chain {} went {nonce_before} -> {nonce}",
                b.chain_id()
            );
            assert_eq!(nonce, nonce_before + 1);
            pool.close().await;
        }
    }

    // ----------------------------------------------------------------------
    // coverage.md section 7 item 2: restart replay with a file-backed database
    // (one local Anvil node at QA2_XC_RPC_C; poc/F2-XC-050/coverage-7.2/run.sh)
    // ----------------------------------------------------------------------
    mod qa2_cov_7_2 {
        use super::{qa2_xc_common::*, *};
        use crate::{
            index::blocks::BlockWatcher,
            provider::Provider,
            tx::{Config as TxConfig, TransactionQueue},
        };
        use alloy::providers::Provider as _;
        use url::Url;

        /// One service "run": initialise the watcher from the store, apply every
        /// queued update (each `New`/`Warp` carries one event per block so that
        /// every block emits one action), reconcile and drain the queue, and
        /// stop. Returns the watcher updates seen and the store status.
        async fn run(label: &str, path: &str, provider: &Provider, signer: &crate::tx::Signer) -> (Vec<BlockUpdate>, BlockStatus) {
            let pool = file_pool(path).await;
            let mut machine = new_machine(&pool).await;
            let indexed = machine.block_status().await.unwrap();
            let mut watcher = BlockWatcher::new(provider.clone(), block_config(2), indexed).await.unwrap();
            let updates = watcher.ready().collect::<Vec<_>>();
            println!("{label}: persisted {indexed:?}; watcher queued {updates:?}");
            let mut queue = TransactionQueue::new(provider.clone(), signer.clone(), pool.clone(), TxConfig::default())
                .await
                .unwrap();
            let mut commands = vec![];
            for update in &updates {
                let events = match update {
                    BlockUpdate::New { number, .. } => vec![*number],
                    BlockUpdate::Warp { from, to } => (*from..=*to).collect(),
                    BlockUpdate::Uncle { .. } => vec![],
                };
                commands.extend(apply(&mut machine, update.clone(), events).await);
            }
            machine.prune(watcher.status().safe).await.unwrap();
            queue.update_block_status(watcher.status()).await.unwrap();
            let transactions = encode(&commands);
            println!("{label}: {} action(s) produced by this run's updates", transactions.len());
            if !transactions.is_empty() {
                queue.queue(transactions).await.unwrap();
            }
            let status = machine.block_status().await.unwrap().unwrap();
            let (_, state) = committed(&pool).await.unwrap();
            println!("{label}: store {status:?}; state events {:?}; transactions {:?}", state.events, transaction_rows(&pool).await);
            drop(queue);
            drop(watcher);
            drop(machine);
            pool.close().await;
            (updates, status)
        }

        #[tokio::test]
        async fn restart_replay_across_a_shallow_reorg_a_long_outage_and_a_reorg_of_the_anchor() {
            let path = fresh_db("cov72");
            let rpc = env_or("QA2_XC_RPC_C", "http://127.0.0.1:8747");
            let provider = Provider::connect(&Url::parse(&rpc).unwrap()).await.unwrap();
            let signer = anvil_signer();
            let me = signer.address();
            println!("rpc {rpc} -> chain_id {}", provider.chain_id());

            // run 1: fresh start; the two recent blocks emit one action each,
            // both submitted (automine puts each into its own block).
            let (updates, status) = run("run 1", &path, &provider, &signer).await;
            assert!(updates.iter().all(|update| matches!(update, BlockUpdate::New { .. })));
            let nonce_after_run_1 = provider.get_transaction_count(me).await.unwrap();
            println!("run 1: signer nonce {nonce_after_run_1}");

            // Offline: a reorg inside `max_reorg_depth` replaces the head
            // (Anvil's replacement blocks are empty, so the submitted
            // transactions are dropped), then one more block.
            // (automine added one block per submitted transaction, so the depth
            // is computed from the live head: replace the store's `latest` and
            // everything above it, one block inside `max_reorg_depth`).
            let head = provider.get_block_number().await.unwrap();
            let depth = head - status.latest + 1;
            let latest_before = block_hash(&provider, status.latest).await;
            anvil(&provider, "anvil_reorg", serde_json::json!([depth, []])).await;
            anvil(&provider, "anvil_mine", serde_json::json!(["0x1"])).await;
            let latest_after = block_hash(&provider, status.latest).await;
            println!(
                "offline: head {head}, anvil_reorg depth {depth} (block {} hash {latest_before} -> {latest_after}) + 1 block; signer nonce now {}",
                status.latest,
                provider.get_transaction_count(me).await.unwrap()
            );
            assert_ne!(latest_before, latest_after);

            // run 2: `Uncle { safe + 1 }` then replay; the replayed blocks'
            // actions are enqueued again next to the still-in-flight rows.
            let (updates, _) = run("run 2 (after shallow reorg)", &path, &provider, &signer).await;
            assert!(matches!(updates[0], BlockUpdate::Uncle { number } if number == status.safe + 1));
            let nonce_after_run_2 = provider.get_transaction_count(me).await.unwrap();
            println!("run 2: signer nonce {nonce_after_run_2}");

            // Offline: a long outage (more blocks than `max_reorg_depth`).
            anvil(&provider, "anvil_mine", serde_json::json!(["0xa"])).await;
            let (updates, status) = run("run 3 (after 10-block outage)", &path, &provider, &signer).await;
            assert!(matches!(updates[0], BlockUpdate::Uncle { .. }));
            assert!(matches!(updates[1], BlockUpdate::Warp { .. }));

            // Offline: a reorg deeper than `max_reorg_depth` that replaces the
            // persisted `safe` anchor itself, then one block.
            let head = provider.get_block_number().await.unwrap();
            let depth = head - status.safe + 1;
            let anchor_before = block_hash(&provider, status.safe).await;
            anvil(&provider, "anvil_reorg", serde_json::json!([depth, []])).await;
            anvil(&provider, "anvil_mine", serde_json::json!(["0x1"])).await;
            let anchor_after = block_hash(&provider, status.safe).await;
            println!("offline: head {head}, anvil_reorg depth {depth}; anchor block {} hash before {anchor_before} after {anchor_after}", status.safe);
            assert_ne!(anchor_before, anchor_after);
            let (updates, _) = run("run 4 (after the anchor was replaced)", &path, &provider, &signer).await;
            assert!(!updates.is_empty(), "restart proceeded without ExceededMaxReorgDepth");
            println!("run 4: signer nonce {}", provider.get_transaction_count(me).await.unwrap());
        }
    }
