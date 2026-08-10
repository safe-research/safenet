//! Shared vocabulary between check logic and the Safenet Arbitration
//! Charter. Every check denial cites a [`RuleId`], so a `Decision`'s
//! human-readable reason is always structurally traceable back to the
//! Charter rule it's justified by, rather than free-form prose.
//!
//! Grown incrementally: a variant is added in the same change that
//! implements the check giving it meaning, not declared upfront as an
//! unused placeholder.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};

/// A specific Safenet Arbitration Charter rule that a check denial maps to.
///
/// Rules are represented as `R-{article}.{section}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleId {
    /// The article for the rule.
    pub article: u8,
    /// The section for the rule.
    pub section: u8,
}

impl RuleId {
    /// Article IV Part A, R-4.1: a self-call must target an allow-listed Safe
    /// settings-management function (owner/threshold/guard/module/fallback
    /// handler changes, or a known singleton migration).
    pub const SETTINGS_CHANGE: Self = Self::new(4, 1);

    /// Article IV Part A, R-4.2: a delegatecall must target a known Safe
    /// migration, signing-library, `CreateCall`, or MultiSend contract,
    /// calling one of that contract's allow-listed functions.
    pub const DELEGATECALL_INTEGRITY: Self = Self::new(4, 2);

    /// Article IV Part B, R-4.6: known malicious or compromised destination
    /// address. MVP note: currently backed only by a static operator
    /// blocklist, not source-attributed threat intel.
    pub const KNOWN_MALICIOUS_TARGET: Self = Self::new(4, 6);

    /// Article IV Part B, R-4.5: an authorization-target grant that is
    /// functionally unlimited — max `uint256` for an ERC-20 `approve`, or an
    /// ERC-721/ERC-1155 "approval for all tokens" (`setApprovalForAll`).
    /// Per §2.5, this sub-case is always functionally unlimited and needs no
    /// further analysis (unlike the rest of §2.5's amount-reasonableness
    /// factors, which remain out of scope for this MVP).
    ///
    /// Also covers two worked CoW Swap instances (`crates/sentinel::cow`):
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
    ///   itself is instead [`Self::AUTHORIZATION_TARGET`] — a
    ///   target-manipulation, not an amount, concern.)
    /// - An exact 2-call presignature batch — the same `approve` plus a
    ///   `setPreSignature` call — whose approved token/amount don't exactly
    ///   match the referenced order's own `sellToken`/`sellAmount`, fetched
    ///   from CoW's public order-by-UID API. A Safe `approve` sets an
    ///   allowance rather than incrementing it, so both an under-approval
    ///   and an over-approval are denied. (As with the TWAP case, an order
    ///   whose proceeds don't go back to the Safe itself is instead
    ///   [`Self::AUTHORIZATION_TARGET`].)
    pub const EXCESSIVE_APPROVAL: Self = Self::new(4, 5);

    /// Article IV Part B, R-4.3: an ERC-20 `transfer`/`transferFrom`
    /// recipient that resembles the address-poisoning pattern §2.4 Notes
    /// names as circumstantial evidence ("the recipient address resembles a
    /// prior user address..."). MVP note: checked dynamically (see
    /// `crate::address_poisoning` in `crates/sentinel`) by looking for a
    /// prior genuine `Transfer` from the Safe to the exact same recipient,
    /// within a bounded, recent block range — not a full history scan, and
    /// not yet a real denial path (see that module's docs).
    pub const VALUE_TARGET: Self = Self::new(4, 3);

    /// Article IV Part B, R-4.4: the same address-poisoning pattern as
    /// [`Self::VALUE_TARGET`], applied to an ERC-20 `approve` spender
    /// (an authorization-target grant) rather than a value transfer.
    ///
    /// Also covers both CoW Swap worked examples (`crates/sentinel::cow`,
    /// TWAP and presignature alike): an order whose receiver isn't the Safe
    /// itself would route the order's proceeds to an unrelated address —
    /// the same target-manipulation concern as a wrong `approve` spender,
    /// distinct from [`Self::EXCESSIVE_APPROVAL`]'s amount-based checks
    /// on those same batches.
    pub const AUTHORIZATION_TARGET: Self = Self::new(4, 4);

    /// Creates a new rule ID for the given article and section.
    pub const fn new(article: u8, section: u8) -> Self {
        Self { article, section }
    }
}

impl Display for RuleId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "R-{}.{}", self.article, self.section)
    }
}

/// An error parsing a rule from a string.
#[derive(Debug, thiserror::Error)]
pub enum ParseRuleError {
    #[error("the rule is not specified in the R-#.# format")]
    InvalidFormat,
    #[error("the rule article or section is not a valid number")]
    InvalidInteger(#[from] ParseIntError),
}

impl FromStr for RuleId {
    type Err = ParseRuleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (article, section) = s
            .strip_prefix("R-")
            .and_then(|s| s.split_once('.'))
            .ok_or(ParseRuleError::InvalidFormat)?;
        Ok(Self {
            article: article.parse()?,
            section: section.parse()?,
        })
    }
}

impl Serialize for RuleId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RuleId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_known_value() {
        for (rule, code) in [
            (RuleId::SETTINGS_CHANGE, "R-4.1"),
            (RuleId::DELEGATECALL_INTEGRITY, "R-4.2"),
            (RuleId::KNOWN_MALICIOUS_TARGET, "R-4.6"),
            (RuleId::EXCESSIVE_APPROVAL, "R-4.5"),
            (RuleId::VALUE_TARGET, "R-4.3"),
            (RuleId::AUTHORIZATION_TARGET, "R-4.4"),
        ] {
            assert_eq!(rule.to_string(), code);
            assert_eq!(RuleId::from_str(code).unwrap(), rule);

            let json = serde_json::to_string(&rule).unwrap();
            assert_eq!(json, format!("\"{code}\""));
            assert_eq!(serde_json::from_str::<RuleId>(&json).unwrap(), rule);
        }
    }

    #[test]
    fn from_str_allows_unknown_codes() {
        assert_eq!(RuleId::from_str("R-4.99").unwrap(), RuleId::new(4, 99));
    }
}
