use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, num::NonZeroU64};

/// An epoch ID.
#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum EpochId {
    /// The genesis epoch.
    #[default]
    Genesis,
    /// A regular epoch established after genesis.
    Number { number: NonZeroU64 },
}

impl EpochId {
    /// Returns the epoch ID for a raw value.
    pub const fn from_raw(value: u64) -> Self {
        match NonZeroU64::new(value) {
            Some(number) => Self::Number { number },
            None => Self::Genesis,
        }
    }

    /// Returns the epoch ID as a raw numerical value. Genesis is represented
    /// by the value 0.
    pub const fn raw_value(self) -> u64 {
        match self {
            EpochId::Genesis => 0,
            EpochId::Number { number } => number.get(),
        }
    }
}

impl PartialOrd for EpochId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EpochId {
    fn cmp(&self, other: &Self) -> Ordering {
        u64::cmp(&self.raw_value(), &other.raw_value())
    }
}
