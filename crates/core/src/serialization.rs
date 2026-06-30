//! Serialization helpers.

/// Deserialization helper to use the [`std::str::FromStr`] implementation to
/// deserialize from a string value.
pub mod from_str {
    use serde::{Deserialize as _, Deserializer, de};
    use std::{borrow::Cow, fmt::Display, str::FromStr};

    #[doc(hidden)]
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        T::Err: Display,
    {
        // Note that we use `Cow<str>` instead of `&str` or `String` here; while
        // we would want a `&str` here since we only need the string temporarily
        // for deserialization. However, not all deserializers support this
        // (notably the JSON deserializer because of JSON string semantics) and
        // only support deserializing in to owned `String`s. Use `Cow` to get a
        // reference if supported, and an owned string otherwise.
        let str = Cow::<'de, str>::deserialize(deserializer)?;
        T::from_str(&str).map_err(de::Error::custom)
    }
}
