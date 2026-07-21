use crate::bindings::consensus::SafeTransaction;
use alloy::primitives::Address;
use std::{borrow::Cow, collections::HashSet};

/// The detector's verdict on a proposed oracle transaction: whether to
/// approve it, and the justification to carry, verbatim, into the blind
/// commit-reveal vote. `reason` is always a static string literal today, so
/// `Cow` avoids allocating one on every `check` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub approve: bool,
    pub reason: Cow<'static, str>,
}

/// Decides whether a proposed oracle transaction should be approved.
///
/// Approves every transaction whose destination is not in the blocklist;
/// mirrors `createDetector` from `sentinel/detector.ts`.
pub struct Detector {
    // The blocklist never changes once the detector is created.
    blocklist: HashSet<Address>,
}

impl Detector {
    pub fn new(blocklist: impl IntoIterator<Item = Address>) -> Self {
        Self {
            blocklist: blocklist.into_iter().collect(),
        }
    }

    pub fn check(&self, tx: &SafeTransaction) -> Decision {
        let approve = !self.blocklist.contains(&tx.to);
        let reason = if approve {
            "destination is not blocklisted"
        } else {
            "destination is blocklisted"
        };
        Decision {
            approve,
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A1: Address = Address::new([1u8; 20]);
    const A2: Address = Address::new([2u8; 20]);
    const A3: Address = Address::new([3u8; 20]);

    fn tx(to: Address) -> SafeTransaction {
        SafeTransaction {
            to,
            ..Default::default()
        }
    }

    #[test]
    fn denied_when_blocklisted() {
        let detector = Detector::new(vec![A1, A2]);
        let decision = detector.check(&tx(A1));
        assert!(!decision.approve);
        assert_eq!(decision.reason, "destination is blocklisted");
        assert!(!detector.check(&tx(A2)).approve);
    }

    #[test]
    fn approved_with_empty_blocklist() {
        let detector = Detector::new(vec![]);
        assert!(detector.check(&tx(A1)).approve);
    }

    #[test]
    fn approved_when_not_blocklisted() {
        let detector = Detector::new(vec![A1, A2]);
        let decision = detector.check(&tx(A3));
        assert!(decision.approve);
        assert_eq!(decision.reason, "destination is not blocklisted");
    }
}
