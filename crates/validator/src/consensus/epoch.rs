use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{cmp::Ordering, num::NonZeroU64};

/// Returns the next epoch number for `block`, given the configured number of
/// blocks per epoch.
pub const fn next_number(block: u64, blocks_per_epoch: NonZeroU64) -> NonZeroU64 {
    let number = block / blocks_per_epoch.get();
    NonZeroU64::MIN.saturating_add(number)
}

/// An epoch ID.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum EpochId {
    /// The genesis epoch.
    #[default]
    Genesis,
    /// A regular epoch established after genesis.
    Number { number: NonZeroU64 },
}

impl Serialize for EpochId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.raw_value())
    }
}

impl<'de> Deserialize<'de> for EpochId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_raw(u64::deserialize(deserializer)?))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_next_epoch_number_from_block() {
        let blocks_per_epoch = NonZeroU64::new(100).unwrap();

        assert_eq!(next_number(0, blocks_per_epoch).get(), 1);
        assert_eq!(next_number(99, blocks_per_epoch).get(), 1);
        assert_eq!(next_number(100, blocks_per_epoch).get(), 2);
        assert_eq!(next_number(199, blocks_per_epoch).get(), 2);
        assert_eq!(
            next_number(u64::MAX, blocks_per_epoch).get(),
            u64::MAX / 100 + 1
        );
    }
}
