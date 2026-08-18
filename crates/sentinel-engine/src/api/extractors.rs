//! Typed extractors for optional security-check headers.

use alloy::primitives::B256;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use std::{str::FromStr, time::Duration};

/// The optional Safenet request ID supplied in `x-request-id`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestId(pub Option<B256>);

impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let request_id = parse_header(parts, "x-request-id", || {
            (
                StatusCode::BAD_REQUEST,
                "x-request-id must be a 0x-prefixed 32-byte digest",
            )
        })?;
        Ok(Self(request_id))
    }
}

/// The optional caller timeout budget supplied in `x-request-timeout`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestTimeout(pub Option<Duration>);

impl<S> FromRequestParts<S> for RequestTimeout
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let timeout = parse_header(parts, "x-request-timeout", || {
            (
                StatusCode::BAD_REQUEST,
                "x-request-timeout must be an unsigned integer number of milliseconds",
            )
        })?;
        Ok(Self(timeout.map(Duration::from_millis)))
    }
}

fn parse_header<T, R>(
    parts: &mut Parts,
    header: &str,
    rejection: impl Fn() -> R,
) -> Result<Option<T>, R>
where
    T: FromStr,
{
    let Some(header) = parts.headers.get(header) else {
        return Ok(None);
    };
    let value = header
        .to_str()
        .map_err(|_| rejection())?
        .parse()
        .map_err(|_| rejection())?;
    Ok(Some(value))
}
