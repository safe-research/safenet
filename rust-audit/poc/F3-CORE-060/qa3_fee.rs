//! QA3 F3-CORE-060 harness: the real `Driver` and `TransactionQueue` (Direct
//! mode, default `blocks_before_resubmit` 2) following a real Anvil chain
//! through `Provider::connect` (HTTP), with one action queued at startup.
//! The RPC URL is a loopback proxy that records `eth_sendRawTransaction` and
//! answers with the hash without forwarding it: an endpoint that accepts but
//! does not propagate.
//!
//! Usage: qa3_fee <rpc url> <seconds> <sqlite path>

use alloy::primitives::{Address, U256};
use safenet_core::{
    Driver,
    driver::{ActionEncoder, Config, Service},
    effects::EffectHandler,
    index::blocks::Timing,
    provider::Provider,
    state::{Commands, Message, StateTransition},
    tx::{Signer, Transaction},
    utils,
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use std::{str::FromStr, time::Duration};

alloy::sol! {
    #[derive(Debug)]
    contract Watched {
        event Ping(uint256 value);
    }
}
safenet_core::watcher_events!(Watched::WatchedEvents);

#[derive(Default, Serialize, Deserialize)]
struct QaState;

struct QaTransition;

impl StateTransition<QaState> for QaTransition {
    type Event = Watched::WatchedEvents;
    type Action = ();
    type Effect = ();
    type Resume = ();

    fn apply_transition(
        &self,
        state: QaState,
        _message: Message<Self::Event, Self::Resume>,
    ) -> (QaState, Commands<QaState, Self>) {
        (state, vec![])
    }
}

struct QaEffects;

impl EffectHandler<(), ()> for QaEffects {
    async fn perform_effect(&self, _effect: ()) {}
}

struct QaActions;

impl ActionEncoder<()> for QaActions {
    fn encode_action(&self, _action: ()) -> (Transaction, Option<u64>) {
        (
            Transaction {
                to: Address::repeat_byte(0xac),
                value: U256::ZERO,
                gas: 21_000,
                ..Default::default()
            },
            None,
        )
    }
}

struct QaService;

impl Service for QaService {
    type State = QaState;
    type Event = Watched::WatchedEvents;
    type Action = ();
    type Effect = ();
    type Resume = ();
    type Transition = QaTransition;
    type Effects = QaEffects;
    type Actions = QaActions;

    fn components(self) -> (QaTransition, QaEffects, QaActions) {
        (QaTransition, QaEffects, QaActions)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,safenet_core::tx=debug".into()),
        )
        .with_ansi(false)
        .init();
    let args = std::env::args().collect::<Vec<_>>();
    let url = args[1].parse().unwrap();
    let seconds = args[2].parse().unwrap();
    let database = &args[3];

    let provider = Provider::connect(&url).await.unwrap();
    let pool = utils::connect_sqlite(
        SqliteConnectOptions::from_str(&format!("sqlite:{database}?mode=rwc")).unwrap(),
    )
    .await
    .unwrap();
    // Anvil's first default account.
    let signer = Signer::from_str(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    )
    .unwrap();
    let mut config = Config::default();
    // Anvil mines a block per second; the resubmission schedule counts blocks.
    config.index.blocks.block_time = Timing::Millis(1_000);
    config.index.blocks.block_propagation_delay = Timing::Millis(0);
    config.index.blocks.block_retry_delays = Timing::Millis(vec![]);
    println!("transaction queue config: {:?}", config.transactions);
    let mut driver = Driver::new(
        QaService,
        provider,
        signer,
        pool,
        vec![Address::repeat_byte(0x11)],
        config,
    )
    .await
    .unwrap();
    driver.queue_action(()).await.unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(seconds), driver.run()).await;
    println!("harness stopped after {seconds} s");
}
