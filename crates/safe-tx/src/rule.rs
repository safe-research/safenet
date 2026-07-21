//! Shared vocabulary between check logic and the Safenet Arbitration
//! Charter. Every check denial cites a [`RuleId`], so a `Decision`'s
//! human-readable reason is always structurally traceable back to the
//! Charter rule it's justified by, rather than free-form prose.
//!
//! Grown incrementally: a variant is added in the same change that
//! implements the check giving it meaning, not declared upfront as an
//! unused placeholder.

/// A specific Safenet Arbitration Charter rule that a check denial maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleId {
    /// Article IV Part A, R-4.1: a self-call must target an allow-listed Safe
    /// settings-management function (owner/threshold/guard/module/fallback
    /// handler changes, or a known singleton migration).
    R4_1SettingsChange,
    /// Article IV Part A, R-4.2: a delegatecall must target a known Safe
    /// migration, signing-library, `CreateCall`, or MultiSend contract,
    /// calling one of that contract's allow-listed functions.
    R4_2DelegatecallIntegrity,
    /// Article IV Part B, R-4.6: known malicious or compromised destination
    /// address. MVP note: currently backed only by a static operator
    /// blocklist, not source-attributed threat intel.
    R4_6KnownMaliciousTarget,
}

impl RuleId {
    /// The rule's canonical Charter citation, e.g. `"R-4.1"`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::R4_1SettingsChange => "R-4.1",
            Self::R4_2DelegatecallIntegrity => "R-4.2",
            Self::R4_6KnownMaliciousTarget => "R-4.6",
        }
    }
}
