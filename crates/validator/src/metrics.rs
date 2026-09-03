//! This validator's own Prometheus metrics.
//!
//! Each metric's name and its expected labels are defined together in one
//! accessor function here, so every place a metric is recorded goes through a
//! typed function that documents and enforces its label shape.

use metrics::Counter;

/// The input to a single validator state transition, as recorded by
/// [`transitions_total`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionKind {
    /// A new block was applied to the state machine.
    Block,
    /// An onchain event was applied to the state machine.
    Event,
    /// An effect result was resumed into the state machine.
    Resume,
}

impl TransitionKind {
    fn variants() -> impl Iterator<Item = Self> {
        [Self::Block, Self::Event, Self::Resume].into_iter()
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Event => "event",
            Self::Resume => "resume",
        }
    }
}

/// Number of validator state transitions applied, by input `kind`.
pub fn transitions_total(kind: TransitionKind) -> Counter {
    let kind = kind.label();
    metrics::counter!("safenet_validator_transitions_total", "kind" => kind)
}

/// The validator effect being performed, as recorded by [`effects_total`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectKind {
    /// Generate and persist the secrets for a key generation ceremony.
    KeyGenSetup,
    /// Start eagerly generating nonce chunks for a group.
    StartNonceGeneration,
    /// Generate and persist the next nonce tree for a group.
    NonceTree,
    /// Reveal this validator's nonce commitments.
    RevealNonceCommitments,
    /// Consume this validator's nonce for a signing round.
    UseNonce,
    /// Reconcile stored group secrets with the state machine.
    ReconcileGroupSecrets,
}

impl EffectKind {
    fn variants() -> impl Iterator<Item = Self> {
        [
            Self::KeyGenSetup,
            Self::StartNonceGeneration,
            Self::NonceTree,
            Self::RevealNonceCommitments,
            Self::UseNonce,
            Self::ReconcileGroupSecrets,
        ]
        .into_iter()
    }

    fn label(&self) -> &'static str {
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

/// The result of a validator effect attempt, as recorded by
/// [`effects_total`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectResult {
    /// The effect completed successfully, including an expected no-op.
    Success,
    /// The effect returned an error.
    Failure,
}

impl EffectResult {
    fn variants() -> impl Iterator<Item = Self> {
        [Self::Success, Self::Failure].into_iter()
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// Number of completed validator effect attempts, by `effect` and `result`.
pub fn effects_total(effect: EffectKind, result: EffectResult) -> Counter {
    let effect = effect.label();
    let result = result.label();
    metrics::counter!(
        "safenet_validator_effects_total",
        "effect" => effect,
        "result" => result,
    )
}

/// Materializes every bounded validator metric series at zero.
pub fn initialize() {
    for kind in TransitionKind::variants() {
        transitions_total(kind).absolute(0);
    }
    for effect in EffectKind::variants() {
        for result in EffectResult::variants() {
            effects_total(effect, result).absolute(0);
        }
    }
}
