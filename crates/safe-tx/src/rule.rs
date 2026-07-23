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
    /// Article IV Part B, R-4.5: an authorization-target grant that is
    /// functionally unlimited — max `uint256` for an ERC-20 `approve`, or an
    /// ERC-721/ERC-1155 "approval for all tokens" (`setApprovalForAll`).
    /// Per §2.5, this sub-case is always functionally unlimited and needs no
    /// further analysis (unlike the rest of §2.5's amount-reasonableness
    /// factors, which remain out of scope for this MVP).
    R4_5ExcessiveApproval,
}

impl RuleId {
    /// The rule's canonical Charter citation, e.g. `"R-4.1"`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::R4_1SettingsChange => "R-4.1",
            Self::R4_2DelegatecallIntegrity => "R-4.2",
            Self::R4_6KnownMaliciousTarget => "R-4.6",
            Self::R4_5ExcessiveApproval => "R-4.5",
        }
    }

    /// Parses a rule's canonical Charter citation back into a [`RuleId`],
    /// e.g. for validating a code an external check service cites in its
    /// response. `None` for anything not among the variants declared so far.
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::R4_1SettingsChange,
            Self::R4_2DelegatecallIntegrity,
            Self::R4_6KnownMaliciousTarget,
            Self::R4_5ExcessiveApproval,
        ]
        .into_iter()
        .find(|rule| rule.code() == code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_code_round_trips_every_variant() {
        for rule in [
            RuleId::R4_1SettingsChange,
            RuleId::R4_2DelegatecallIntegrity,
            RuleId::R4_6KnownMaliciousTarget,
            RuleId::R4_5ExcessiveApproval,
        ] {
            assert_eq!(RuleId::from_code(rule.code()), Some(rule));
        }
    }

    #[test]
    fn from_code_rejects_unknown_codes() {
        assert_eq!(RuleId::from_code("R-4.99"), None);
    }
}
