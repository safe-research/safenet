//! Prometheus metrics owned by the validator process.

use metrics::{Counter, Gauge};
use safenet_core::{
    driver::{Metrics as DriverMetrics, ProcessingCursor},
    index::BlockStatus,
    provider::{RpcObserver, RpcOutcome},
};

const BLOCK_NUMBER: &str = "validator_block_number";
const EVENT_INDEX: &str = "validator_event_index";
const REORGS: &str = "validator_reorgs";
const TRANSITIONS: &str = "validator_transitions";
const RPC_REQUESTS: &str = "validator_rpc_requests";
const EFFECTS: &str = "validator_effects";

/// Installs descriptions and materializes every bounded metric series.
pub fn init() {
    metrics::describe_gauge!(
        BLOCK_NUMBER,
        "Block number by processing stage (seen: received from chain, processed: applied to state machine)"
    );
    metrics::describe_gauge!(
        EVENT_INDEX,
        "Event index by processing stage (seen: received from chain, processed: applied to state machine)"
    );
    metrics::describe_counter!(REORGS, "Number of chain reorgs observed by the validator");
    metrics::describe_counter!(TRANSITIONS, "Validator state transitions by input kind");
    metrics::describe_counter!(RPC_REQUESTS, "RPC requests by method and result");
    metrics::describe_counter!(EFFECTS, "Validator effects by type and result");

    for stage in [ProcessingStage::Seen, ProcessingStage::Processed] {
        block_number(stage).set(0.0);
        event_index(stage).set(-1.0);
    }
    reorgs().absolute(0);
    for kind in TransitionKind::ALL {
        transitions(kind).absolute(0);
    }
    for effect in EffectKind::ALL {
        for outcome in [Outcome::Success, Outcome::Failure] {
            effects(effect, outcome).absolute(0);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessingStage {
    Seen,
    Processed,
}

impl ProcessingStage {
    const fn label(self) -> &'static str {
        match self {
            Self::Seen => "seen",
            Self::Processed => "processed",
        }
    }
}

fn block_number(stage: ProcessingStage) -> Gauge {
    metrics::gauge!(BLOCK_NUMBER, "status" => stage.label())
}

fn event_index(stage: ProcessingStage) -> Gauge {
    metrics::gauge!(EVENT_INDEX, "status" => stage.label())
}

fn set_cursor(stage: ProcessingStage, cursor: ProcessingCursor) {
    // Prometheus gauges are f64. Ethereum block and log indexes remain exactly
    // representable at all realistic chain heights.
    block_number(stage).set(cursor.block as f64);
    event_index(stage).set(cursor.event_index.map_or(-1.0, |index| index as f64));
}

fn reorgs() -> Counter {
    metrics::counter!(REORGS)
}

/// The kind of pure validator state transition being applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionKind {
    Block,
    Event,
    Resume,
}

impl TransitionKind {
    const ALL: [Self; 3] = [Self::Block, Self::Event, Self::Resume];

    const fn label(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Event => "event",
            Self::Resume => "resume",
        }
    }
}

/// Records an applied pure state transition.
pub fn transition(kind: TransitionKind) {
    transitions(kind).increment(1);
}

fn transitions(kind: TransitionKind) -> Counter {
    metrics::counter!(TRANSITIONS, "kind" => kind.label())
}

/// A bounded success/failure result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Success,
    Failure,
}

impl Outcome {
    const fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// Stable validator effect labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectKind {
    KeyGenSetup,
    StartNonceGeneration,
    NonceTree,
    RevealNonceCommitments,
    UseNonce,
    ReconcileGroupSecrets,
}

impl EffectKind {
    const ALL: [Self; 6] = [
        Self::KeyGenSetup,
        Self::StartNonceGeneration,
        Self::NonceTree,
        Self::RevealNonceCommitments,
        Self::UseNonce,
        Self::ReconcileGroupSecrets,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::KeyGenSetup => "key_gen_setup",
            Self::StartNonceGeneration => "start_nonce_generation",
            Self::NonceTree => "nonce_tree",
            Self::RevealNonceCommitments => "reveal_nonce_commitments",
            Self::UseNonce => "use_nonce",
            Self::ReconcileGroupSecrets => "reconcile_group_secrets",
        }
    }
}

/// Records a completed validator effect attempt.
pub fn effect(effect: EffectKind, outcome: Outcome) {
    effects(effect, outcome).increment(1);
}

fn effects(effect: EffectKind, outcome: Outcome) -> Counter {
    metrics::counter!(
        EFFECTS,
        "effect" => effect.label(),
        "result" => outcome.label(),
    )
}

fn rpc_request(method: &str, outcome: RpcOutcome) {
    let result = match outcome {
        RpcOutcome::Success => Outcome::Success,
        RpcOutcome::Failure => Outcome::Failure,
    };
    let opposite = match result {
        Outcome::Success => Outcome::Failure,
        Outcome::Failure => Outcome::Success,
    };
    rpc_requests(method, opposite).absolute(0);
    rpc_requests(method, result).increment(1);
}

fn rpc_requests(method: &str, outcome: Outcome) -> Counter {
    metrics::counter!(
        RPC_REQUESTS,
        "method" => method.to_owned(),
        "result" => outcome.label(),
    )
}

/// Creates the provider callback for validator-owned RPC metrics.
pub fn rpc_observer() -> RpcObserver {
    RpcObserver::new(rpc_request)
}

/// Validator-owned observations emitted by the shared driver.
#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics;

impl DriverMetrics for Metrics {
    fn initialize(&self, status: Option<BlockStatus>) {
        if let Some(status) = status {
            set_cursor(
                ProcessingStage::Processed,
                ProcessingCursor {
                    block: status.latest,
                    event_index: None,
                },
            );
        }
    }

    fn update_seen(&self, cursor: ProcessingCursor) {
        set_cursor(ProcessingStage::Seen, cursor);
    }

    fn update_processed(&self, cursor: ProcessingCursor) {
        set_cursor(ProcessingStage::Processed, cursor);
    }

    fn reorg(&self) {
        reorgs().increment(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    fn metric_value<'a>(
        values: &'a [(
            metrics_util::CompositeKey,
            Option<metrics::Unit>,
            Option<metrics::SharedString>,
            DebugValue,
        )],
        name: &str,
        labels: &[(&str, &str)],
    ) -> &'a DebugValue {
        &values
            .iter()
            .find(|(key, _, _, _)| {
                key.key().name() == name
                    && key
                        .key()
                        .labels()
                        .map(|label| (label.key(), label.value()))
                        .eq(labels.iter().copied())
            })
            .unwrap_or_else(|| panic!("missing metric {name} with labels {labels:?}"))
            .3
    }

    #[test]
    fn all_bounded_labels_match_the_metrics_contract() {
        assert_eq!(
            TransitionKind::ALL.map(TransitionKind::label),
            ["block", "event", "resume"]
        );
        assert_eq!(
            EffectKind::ALL.map(EffectKind::label),
            [
                "key_gen_setup",
                "start_nonce_generation",
                "nonce_tree",
                "reveal_nonce_commitments",
                "use_nonce",
                "reconcile_group_secrets",
            ]
        );
        assert_eq!(Outcome::Success.label(), "success");
        assert_eq!(Outcome::Failure.label(), "failure");
    }

    #[test]
    fn records_described_metrics_with_expected_labels_and_values() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        ::metrics::with_local_recorder(&recorder, || {
            init();
            let metrics = Metrics;
            metrics.initialize(Some(BlockStatus {
                latest: 40,
                safe: 35,
            }));
            metrics.update_seen(ProcessingCursor {
                block: 41,
                event_index: Some(7),
            });
            metrics.update_processed(ProcessingCursor {
                block: 41,
                event_index: None,
            });
            metrics.reorg();
            transition(TransitionKind::Block);
            transition(TransitionKind::Event);
            transition(TransitionKind::Resume);
            effect(EffectKind::UseNonce, Outcome::Success);
            effect(EffectKind::UseNonce, Outcome::Failure);
            rpc_request("eth_call", RpcOutcome::Success);
        });

        let values = snapshotter.snapshot().into_vec();
        assert_eq!(
            metric_value(&values, BLOCK_NUMBER, &[("status", "seen")]),
            &DebugValue::Gauge(41.0.into())
        );
        assert_eq!(
            metric_value(&values, EVENT_INDEX, &[("status", "seen")]),
            &DebugValue::Gauge(7.0.into())
        );
        assert_eq!(
            metric_value(&values, BLOCK_NUMBER, &[("status", "processed")]),
            &DebugValue::Gauge(41.0.into())
        );
        assert_eq!(
            metric_value(&values, EVENT_INDEX, &[("status", "processed")]),
            &DebugValue::Gauge((-1.0).into())
        );
        assert_eq!(metric_value(&values, REORGS, &[]), &DebugValue::Counter(1));
        assert_eq!(
            metric_value(&values, TRANSITIONS, &[("kind", "block")]),
            &DebugValue::Counter(1)
        );
        assert_eq!(
            metric_value(&values, TRANSITIONS, &[("kind", "event")]),
            &DebugValue::Counter(1)
        );
        assert_eq!(
            metric_value(&values, TRANSITIONS, &[("kind", "resume")]),
            &DebugValue::Counter(1)
        );
        assert_eq!(
            metric_value(
                &values,
                EFFECTS,
                &[("effect", "use_nonce"), ("result", "success")],
            ),
            &DebugValue::Counter(1)
        );
        assert_eq!(
            metric_value(
                &values,
                RPC_REQUESTS,
                &[("method", "eth_call"), ("result", "failure")],
            ),
            &DebugValue::Counter(0)
        );
        assert_eq!(
            metric_value(
                &values,
                EFFECTS,
                &[("effect", "use_nonce"), ("result", "failure")],
            ),
            &DebugValue::Counter(1)
        );
        assert_eq!(
            metric_value(
                &values,
                RPC_REQUESTS,
                &[("method", "eth_call"), ("result", "success")],
            ),
            &DebugValue::Counter(1)
        );

        let allowed = [
            BLOCK_NUMBER,
            EVENT_INDEX,
            REORGS,
            TRANSITIONS,
            RPC_REQUESTS,
            EFFECTS,
        ];
        assert!(values.iter().all(
            |(key, _, description, _)| allowed.contains(&key.key().name()) && description.is_some()
        ));
    }

    #[test]
    fn initializes_the_processed_cursor_from_persisted_state() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        ::metrics::with_local_recorder(&recorder, || {
            init();
            Metrics.initialize(Some(BlockStatus {
                latest: 47918937,
                safe: 47918932,
            }));
        });

        let values = snapshotter.snapshot().into_vec();
        assert_eq!(
            metric_value(&values, BLOCK_NUMBER, &[("status", "processed")]),
            &DebugValue::Gauge(47918937.0.into())
        );
        assert_eq!(
            metric_value(&values, EVENT_INDEX, &[("status", "processed")]),
            &DebugValue::Gauge((-1.0).into())
        );
    }
}
