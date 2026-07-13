use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{cmp::Ordering, num::NonZeroU64};

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
