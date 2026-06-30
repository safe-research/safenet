use std::collections::HashSet;

use alloy::primitives::Address;

use crate::bindings::consensus::SafeTransaction;

/// Decides whether a proposed oracle transaction should be approved.
///
/// Approves every transaction whose destination is not in the blocklist;
/// mirrors `createDetector` from `sentinel/detector.ts`.
pub struct Detector {
    blocklist: HashSet<Address>,
}

impl Detector {
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn new(blocklist: impl IntoIterator<Item = Address>) -> Self {
        Self {
            blocklist: blocklist.into_iter().collect(),
        }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn approve(&self, tx: &SafeTransaction) -> bool {
        !self.blocklist.contains(&tx.to)
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
        assert!(!detector.approve(&tx(A1)));
        assert!(!detector.approve(&tx(A2)));
    }

    #[test]
    fn approved_with_empty_blocklist() {
        let detector = Detector::new(vec![]);
        assert!(detector.approve(&tx(A1)));
    }

    #[test]
    fn approved_when_not_blocklisted() {
        let detector = Detector::new(vec![A1, A2]);
        assert!(detector.approve(&tx(A3)));
    }
}
