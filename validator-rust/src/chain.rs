use serde::{Deserialize, Deserializer, de};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Gnosis,
    Sepolia,
    Alloy,
}

impl<'de> Deserialize<'de> for Chain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "gnosis" => Ok(Self::Gnosis),
            "sepolia" => Ok(Self::Sepolia),
            "alloy" => Ok(Self::Alloy),
            _ => Err(de::Error::unknown_variant(
                &value,
                &["gnosis", "sepolia", "alloy"],
            )),
        }
    }
}
