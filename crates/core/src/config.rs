//! Loading of service TOML configuration files.

use serde::de::DeserializeOwned;
use std::{
    fmt::{self, Display, Formatter},
    path::Path,
};
use tokio::{fs, io};

/// Error produced when loading a configuration file.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An IO error when reading the file.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Error when parsing the configuration.
    #[error(transparent)]
    Parse(#[from] ParseError),
}

/// An error parsing a configuration file.
///
/// The error never contains the contents of the configuration file, which
/// holds secrets such as the signer's private key or an RPC URL with an API
/// key, so it is safe to log.
#[derive(Debug)]
pub struct ParseError {
    position: Option<(usize, usize)>,
    inner: toml::de::Error,
}

impl ParseError {
    fn new(contents: &str, mut inner: toml::de::Error) -> Self {
        // The `toml` error keeps a copy of the whole document, which it prints
        // as context and includes in its `Debug` output. Keep only the
        // position of the error.
        inner.set_input(None);
        let position = inner.span().map(|span| {
            let before = &contents[..span.start.min(contents.len())];
            let line = before.matches('\n').count() + 1;
            let column = before[before.rfind('\n').map_or(0, |i| i + 1)..]
                .chars()
                .count()
                + 1;
            (line, column)
        });
        Self { position, inner }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Some((line, column)) = self.position {
            writeln!(f, "TOML parse error at line {line}, column {column}")?;
        }
        write!(f, "{}", self.inner.to_string().trim_end())
    }
}

impl std::error::Error for ParseError {}

/// Loads a configuration from a TOML file.
pub async fn load<T>(file: &Path) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let contents = fs::read_to_string(file).await?;
    parse(&contents)
}

fn parse<T>(contents: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    toml::from_str(contents).map_err(|err| Error::Parse(ParseError::new(contents, err)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::Signer;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Config {
        rpc: String,
        signer: Signer,
        count: u64,
    }

    const SECRET: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn parse_error(contents: &str) -> Error {
        parse::<Config>(contents).unwrap_err()
    }

    #[test]
    fn parses_configuration() {
        let config = parse::<Config>(&format!(
            r#"
                rpc = "https://rpc.example/v3/apikey"
                signer = "0x{SECRET}"
                count = 1
            "#
        ))
        .unwrap();
        assert_eq!(config.count, 1);
    }

    #[test]
    fn errors_do_not_contain_the_configuration() {
        for (contents, position, message) in [
            (
                // An invalid value in the line with the secret.
                format!(
                    "rpc = \"https://rpc.example/v3/apikey\"\n\
                     signer = \"0x{}\"\n\
                     count = 1\n",
                    &SECRET[1..],
                ),
                "line 2, column 10",
                "in `signer`",
            ),
            (
                // An invalid value elsewhere in the file.
                format!(
                    "rpc = \"https://rpc.example/v3/apikey\"\n\
                     signer = \"0x{SECRET}\"\n\
                     count = -1\n"
                ),
                "line 3, column 9",
                "in `count`",
            ),
            (
                // A syntax error.
                format!(
                    "rpc = \"https://rpc.example/v3/apikey\"\n\
                     signer = \"0x{SECRET}\n\
                     count = 1\n"
                ),
                "line 2, column",
                "",
            ),
        ] {
            let err = parse_error(&contents);
            for output in [err.to_string(), format!("{err:?}")] {
                assert!(!output.contains(&SECRET[4..60]), "{output}");
                assert!(!output.contains("apikey"), "{output}");
            }
            let display = err.to_string();
            assert!(display.contains(position), "{display}");
            assert!(display.contains(message), "{display}");
        }
    }
}
