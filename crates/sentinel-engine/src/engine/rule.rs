//! Shared vocabulary between check logic and the Safenet Arbitration
//! Charter. Every check denial cites a [`RuleId`], so a `Decision`'s
//! human-readable reason is always structurally traceable back to the
//! Charter rule it's justified by, rather than free-form prose.
//!
//! Grown incrementally: a variant is added in the same change that
//! implements the check giving it meaning, not declared upfront as an
//! unused placeholder.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::borrow::Cow;

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
    ///
    /// Also covers two worked CoW Swap instances
    /// ([`crate::checkers::CowChecker`]):
    ///
    /// - An exact 2-call TWAP batch — an ERC-20 `approve` to CoW's
    ///   `GPv2VaultRelayer` plus a TWAP order's `create` call — whose
    ///   approved token doesn't match the order's own sell token, or whose
    ///   approved amount exceeds the order's total sell amount
    ///   (`partSellAmount * n`) — both decoded straight from `create`'s
    ///   calldata, no RPC needed, since the order's terms are committed
    ///   onchain at creation time. An approval too small to fully fund the
    ///   order is a trade-soundness concern, not a security one, and isn't
    ///   denied by this check. (An order whose receiver isn't the Safe
    ///   itself is instead [`Self::R4_4AuthorizationTarget`] — a
    ///   target-manipulation, not an amount, concern.)
    /// - An exact 2-call presignature batch — the same `approve` plus a
    ///   `setPreSignature` call — whose approved token/amount don't exactly
    ///   match the referenced order's own `sellToken`/`sellAmount`, fetched
    ///   from CoW's public order-by-UID API. A Safe `approve` sets an
    ///   allowance rather than incrementing it, so both an under-approval
    ///   and an over-approval are denied. (As with the TWAP case, an order
    ///   whose proceeds don't go back to the Safe itself is instead
    ///   [`Self::R4_4AuthorizationTarget`].)
    R4_5ExcessiveApproval,
    /// Article IV Part B, R-4.3: an ERC-20 `transfer`/`transferFrom`
    /// recipient that resembles the address-poisoning pattern §2.4 Notes
    /// names as circumstantial evidence ("the recipient address resembles a
    /// prior user address..."). MVP note: checked dynamically by
    /// [`crate::checkers::AddressPoisoningChecker`] against the Safe's own
    /// genuine `Transfer` history on that token within a bounded, recent
    /// block range (not a full history scan) — an exact-match recipient
    /// passes, a lookalike of a different established recipient is denied
    /// under this rule, and a recipient with no history to compare against
    /// still only abstains (see that module's docs).
    R4_3ValueTarget,
    /// Article IV Part B, R-4.4: the same address-poisoning pattern as
    /// [`Self::R4_3ValueTarget`], applied to an ERC-20 `approve` spender
    /// (an authorization-target grant) rather than a value transfer.
    ///
    /// Also covers both CoW Swap worked examples
    /// ([`crate::checkers::CowChecker`], TWAP and presignature alike): an
    /// order whose receiver isn't the Safe itself would route the order's
    /// proceeds to an unrelated address —
    /// the same target-manipulation concern as a wrong `approve` spender,
    /// distinct from [`Self::R4_5ExcessiveApproval`]'s amount-based checks
    /// on those same batches.
    R4_4AuthorizationTarget,
}

impl Serialize for RuleId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for RuleId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code = Cow::<'de, str>::deserialize(deserializer)?;
        Self::from_code(&code)
            .ok_or_else(|| de::Error::custom(format_args!("unrecognized rule code `{code}`")))
    }
}

impl RuleId {
    /// The rule's canonical Charter citation, e.g. `"R-4.1"`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::R4_1SettingsChange => "R-4.1",
            Self::R4_2DelegatecallIntegrity => "R-4.2",
            Self::R4_6KnownMaliciousTarget => "R-4.6",
            Self::R4_5ExcessiveApproval => "R-4.5",
            Self::R4_3ValueTarget => "R-4.3",
            Self::R4_4AuthorizationTarget => "R-4.4",
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
            Self::R4_3ValueTarget,
            Self::R4_4AuthorizationTarget,
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
            RuleId::R4_3ValueTarget,
            RuleId::R4_4AuthorizationTarget,
        ] {
            assert_eq!(RuleId::from_code(rule.code()), Some(rule));
            let json = serde_json::to_string(&rule).unwrap();
            assert_eq!(json, format!("\"{}\"", rule.code()));
            assert_eq!(serde_json::from_str::<RuleId>(&json).unwrap(), rule);
        }
    }

    #[test]
    fn from_code_rejects_unknown_codes() {
        assert_eq!(RuleId::from_code("R-4.99"), None);
        let error = serde_json::from_str::<RuleId>(r#""R-4.99""#).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unrecognized rule code `R-4.99`")
        );
    }
}
