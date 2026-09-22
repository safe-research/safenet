//! This sentinel engine's own Prometheus metrics.
//!
//! Each metric's name and its expected labels are defined together in one
//! accessor function here, rather than split between a name constant at one
//! call site and an ad hoc label list at another -- so every place a metric
//! is recorded goes through a typed function that documents (and enforces)
//! its label shape instead of a raw string.

use crate::engine::{CallCoverage, CoverageLabel};
use metrics::Counter;

/// Tally of missing coverage on an engine abstention, by `aspect` --
/// incremented once per aspect still missing after the fold, so a two-aspect
/// shortfall counts twice. The input to prioritizing which check to add or
/// narrow next; see "Verdict Composition" in the engine guide.
pub fn missing_coverage_total(aspect: CoverageLabel) -> Counter {
    metrics::counter!(
        description: "Number of times an aspect was missing coverage on an engine abstention.",
        "safenet_sentinel_engine_missing_coverage_total",
        "aspect" => aspect.as_str(),
    )
}

/// Registers every possible `aspect` label at zero so the metric appears in
/// scrapes from the start -- Prometheus can't tell "zero occurrences so far"
/// from "counter never created", and an operator diffing dashboards across
/// deploys wants the former.
pub fn init() {
    for label in CallCoverage::all_labels() {
        missing_coverage_total(label).absolute(0);
    }
}
