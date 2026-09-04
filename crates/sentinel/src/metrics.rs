//! This sentinel's own Prometheus metrics.
//!
//! Each metric's name and its expected labels are defined together in one
//! accessor function here, rather than split between a name constant at one
//! call site and an ad hoc label list at another -- so every place a metric
//! is recorded goes through a typed function that documents (and enforces)
//! its label shape instead of a raw string.

use metrics::{Counter, Histogram};

/// The outcome of a single sentinel engine check, as recorded by
/// [`engine_check_verdicts_total`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineCheckVerdict {
    /// The engine considers the transaction secure.
    Secure,
    /// The engine denied the transaction, citing a Charter rule.
    Insecure,
    /// The engine had no trustworthy verdict either way.
    Abstain,
    /// The check itself failed -- a transport failure, timeout, non-2xx
    /// status, or unparseable response.
    Error,
}

impl EngineCheckVerdict {
    fn label(&self) -> &'static str {
        match self {
            Self::Secure => "secure",
            Self::Insecure => "insecure",
            Self::Abstain => "abstain",
            Self::Error => "error",
        }
    }
}

/// Tally of every sentinel engine check outcome, by `verdict`. `error`'s
/// share of the total is this sentinel's engine error rate.
pub fn engine_check_verdicts_total(verdict: EngineCheckVerdict) -> Counter {
    let label = verdict.label();
    metrics::counter!(
        description: "Number of sentinel engine checks, by verdict.",
        "safenet_sentinel_engine_check_verdicts_total",
        "verdict" => label,
    )
}

/// How many proposed requests this sentinel has started tracking (one per
/// unique `TransactionProposed`/request id) -- the denominator for this
/// sentinel's participation rate.
pub fn requests_proposed_total() -> Counter {
    metrics::counter!(
        description: "Number of unique proposed requests tracked by this sentinel.",
        "safenet_sentinel_requests_proposed_total",
    )
}

/// How many requests this sentinel actually committed a bond to (its own
/// `Committed` landing onchain) -- the numerator for its participation rate.
pub fn requests_participated_total() -> Counter {
    metrics::counter!(
        description: "Number of requests this sentinel committed a bond to.",
        "safenet_sentinel_requests_participated_total",
    )
}

/// This sentinel's own bond amount, recorded once per request it commits to.
pub fn bond_amount() -> Histogram {
    metrics::histogram!(
        description: "Bond amount committed by this sentinel per participated request.",
        "safenet_sentinel_bond_amount",
    )
}

/// How a request this sentinel participated in was ultimately resolved, as
/// recorded by [`requests_resolved_total`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedOutcome {
    /// Won without a dispute: this sentinel's own reveal landed onchain and
    /// the request resolved unanimously in its favor.
    Unanimous,
    /// Nobody revealed; bonds reclaimed.
    Timeout,
    /// This sentinel committed a bond but its own reveal never landed
    /// onchain before the request resolved unanimously anyway -- the bond
    /// is slashed like any other non-revealer once a side is established,
    /// so this is a loss, not a win, despite the request resolving without
    /// a dispute.
    #[expect(dead_code)]
    RevealMissed,
    /// This sentinel's vote matched the arbitrated outcome.
    DisputeWon,
    /// This sentinel's vote didn't match the arbitrated outcome.
    DisputeLost,
}

impl ResolvedOutcome {
    fn label(&self) -> &'static str {
        match self {
            Self::Unanimous => "unanimous",
            Self::Timeout => "timeout",
            Self::RevealMissed => "reveal_missed",
            Self::DisputeWon => "dispute_won",
            Self::DisputeLost => "dispute_lost",
        }
    }
}

/// How every request this sentinel participated in was ultimately resolved,
/// by `outcome`. `dispute_won + dispute_lost` over the total is how often a
/// request this sentinel voted on ended up arbitrated; `unanimous +
/// dispute_won` over the total is this sentinel's win rate; `reveal_missed`
/// is neither -- a bond that was committed but slashed for never revealing.
pub fn requests_resolved_total(outcome: ResolvedOutcome) -> Counter {
    let label = outcome.label();
    metrics::counter!(
        description: "Number of participated requests resolved, by outcome.",
        "safenet_sentinel_requests_resolved_total",
        "outcome" => label,
    )
}

/// The amount of this sentinel's own bond slashed by a lost, arbitrated
/// dispute.
pub fn dispute_bond_slashed_amount() -> Histogram {
    metrics::histogram!(
        description: "Bond amount slashed from this sentinel per lost arbitrated dispute.",
        "safenet_sentinel_dispute_bond_slashed_amount",
    )
}

/// This sentinel's own fee reward from a winning claim -- the request's fee,
/// after the DAO's cut, split evenly across every sentinel on the winning
/// side (`SentinelOracleRequests.calcFeeReward`). Only nonzero rewards
/// should be recorded; a losing or timed-out claim's `feeReward` is always
/// `0` and would just be dead weight in this distribution
/// (`requests_resolved_total` already tracks how often this sentinel
/// doesn't win).
pub fn fee_reward_amount() -> Histogram {
    metrics::histogram!(
        description: "Nonzero fee reward amount received by this sentinel per winning claim.",
        "safenet_sentinel_fee_reward_amount",
    )
}
