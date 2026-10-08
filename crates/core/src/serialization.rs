//! Serialization helpers.

/// Deserialization helper to use the [`std::str::FromStr`] implementation to
/// deserialize from a string value.
pub mod from_str {
    use serde::{Deserialize as _, Deserializer, Serializer, de};
    use std::{borrow::Cow, fmt::Display, str::FromStr};

    #[doc(hidden)]
    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Display,
    {
        serializer.collect_str(value)
    }

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

/// Deserialization helper to use the [`std::str::FromStr`] implementation to
/// deserialize from a string value, after substituting environment variables
/// into it.
///
/// Every `${NAME}` in the string is replaced with the value of the `NAME`
/// environment variable, and every `$$` with a literal `$`. Variable names must
/// match `[A-Za-z_][A-Za-z0-9_]*`. Deserialization fails if a referenced
/// variable is not set, or if a `$` is not part of one of these two forms.
///
/// This allows secrets (such as private keys or API keys embedded in URLs) to
/// be provided through the environment instead of being stored in plain text.
/// Only fields that opt in with `#[serde(with = "...::from_str_with_env")]` are
/// substituted, so a configuration file is never interpolated as a whole.
pub mod from_str_with_env {
    use k256::elliptic_curve::zeroize::Zeroizing;
    use serde::{Deserialize as _, Deserializer, de};
    use std::{
        borrow::Cow,
        env::{self, VarError},
        fmt::Display,
        str::FromStr,
    };

    #[doc(hidden)]
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        T::Err: Display,
    {
        // See `from_str::deserialize` for why we use `Cow<str>`.
        let str = Cow::<'de, str>::deserialize(deserializer)?;
        let str = substitute(&str, |name| env::var(name))?;
        T::from_str(&str).map_err(de::Error::custom)
    }

    /// Substitutes variables in `value`, looking up their values with `var`.
    ///
    /// The result is zeroized on drop, as it may contain secrets.
    pub(super) fn substitute<E>(
        value: &str,
        var: impl Fn(&str) -> Result<String, VarError>,
    ) -> Result<Zeroizing<String>, E>
    where
        E: de::Error,
    {
        let mut result = Zeroizing::new(String::with_capacity(value.len()));
        let mut rest = value;
        while let Some(index) = rest.find('$') {
            push(&mut result, &rest[..index]);
            rest = &rest[index + 1..];
            if let Some(tail) = rest.strip_prefix('$') {
                push(&mut result, "$");
                rest = tail;
            } else if let Some(tail) = rest.strip_prefix('{') {
                let (name, tail) = tail
                    .split_once('}')
                    .ok_or_else(|| E::custom("unterminated environment variable reference"))?;
                if !is_valid_name(name) {
                    return Err(E::custom(format!(
                        "invalid environment variable name {name:?}"
                    )));
                }
                // Note that we intentionally don't format the `VarError`, as
                // its `NotUnicode` variant would include the variable's value
                // which may be a secret.
                let value = Zeroizing::new(var(name).map_err(|err| match err {
                    VarError::NotPresent => {
                        E::custom(format!("environment variable {name} is not set"))
                    }
                    VarError::NotUnicode(_) => {
                        E::custom(format!("environment variable {name} is not valid unicode"))
                    }
                })?);
                push(&mut result, &value);
                rest = tail;
            } else {
                return Err(E::custom(
                    "unexpected `$`, use `${NAME}` to reference an environment variable or `$$` \
                     for a literal `$`",
                ));
            }
        }
        push(&mut result, rest);
        Ok(result)
    }

    fn push(buffer: &mut Zeroizing<String>, str: &str) {
        // Grow into a new allocation instead of letting `String::push_str`
        // reallocate, as reallocating would free the old buffer without
        // zeroizing it. Replacing the buffer drops (and zeroizes) the old one.
        if buffer.capacity() - buffer.len() < str.len() {
            let capacity = (buffer.len() + str.len()).max(buffer.capacity() * 2);
            let mut grown = Zeroizing::new(String::with_capacity(capacity));
            grown.push_str(buffer);
            *buffer = grown;
        }
        buffer.push_str(str);
    }

    fn is_valid_name(name: &str) -> bool {
        let mut chars = name.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::Error;
    use std::{env::VarError, ffi::OsString};

    fn substitute(value: &str) -> Result<String, String> {
        from_str_with_env::substitute::<Error>(value, |name| match name {
            "FOO" => Ok("foo".to_owned()),
            "DOLLAR" => Ok("${FOO}$$".to_owned()),
            "LONG" => Ok("x".repeat(1000)),
            "NOT_UNICODE" => Err(VarError::NotUnicode(OsString::from("secret"))),
            _ => Err(VarError::NotPresent),
        })
        .map(|result| result.to_string())
        .map_err(|err| err.to_string())
    }

    #[test]
    fn substitutes_environment_variables() {
        for (value, expected) in [
            ("", ""),
            ("plain", "plain"),
            ("${FOO}", "foo"),
            ("a${FOO}b${FOO}c", "afoobfooc"),
            ("${FOO}${FOO}", "foofoo"),
            ("$$", "$"),
            ("a$$b$$$$", "a$b$$"),
            ("$${FOO}", "${FOO}"),
            ("$$${FOO}", "$foo"),
        ] {
            assert_eq!(substitute(value).unwrap(), expected, "{value:?}");
        }
    }

    #[test]
    fn does_not_substitute_variable_values() {
        assert_eq!(substitute("${DOLLAR}").unwrap(), "${FOO}$$");
    }

    #[test]
    fn substitutes_values_longer_than_the_input() {
        let expected = format!("a{}b{}", "x".repeat(1000), "x".repeat(1000));
        assert_eq!(substitute("a${LONG}b${LONG}").unwrap(), expected);
    }

    #[test]
    fn rejects_invalid_references() {
        for value in [
            "$",
            "a$b",
            "trailing$",
            "${",
            "${FOO",
            "${}",
            "${1FOO}",
            "${FO-O}",
            "${ FOO }",
        ] {
            assert!(substitute(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn rejects_missing_variables() {
        assert!(substitute("${MISSING}").is_err());
    }

    #[test]
    fn does_not_leak_variable_values_in_errors() {
        let err = substitute("${NOT_UNICODE}").unwrap_err();
        assert!(!err.contains("secret"), "{err}");
    }
}
