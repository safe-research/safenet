use alloy::primitives::Address;

/// A Safenet FROST error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An unexpected FROST error.
    ///
    /// This occurs the FROST implementation is called with unexpected inputs
    /// (such as invalid threshold or incorrect participants for a round).
    #[error("unexpected FROST error: {0}")]
    Unexpected(frost_secp256k1::Error),
    /// A participant misbehaved and provided invalid inputs.
    ///
    /// This occurs if an invalid commitment or secret share is provided by
    /// another participant; they should be removed from the participant group.
    #[error("misbehaving participant {culprit}: {cause}")]
    Participant {
        cause: frost_secp256k1::Error,
        culprit: Address,
    },
}

pub(super) trait Culprit<T> {
    fn err_unexpected(self) -> Result<T, Error>;
    fn err_with_culprit(self, participant: Address) -> Result<T, Error>;
}

impl<T> Culprit<T> for Result<T, frost_secp256k1::Error> {
    fn err_unexpected(self) -> Result<T, Error> {
        self.map_err(Error::Unexpected)
    }

    fn err_with_culprit(self, culprit: Address) -> Result<T, Error> {
        self.map_err(|cause| Error::Participant { cause, culprit })
    }
}

/// An error indicating a malformed scalar.
pub(super) const fn malformed_scalar() -> frost_secp256k1::Error {
    frost_secp256k1::Error::FieldError(frost_secp256k1::FieldError::MalformedScalar)
}

/// An error indicating a malformed group element (point).
pub(super) const fn malformed_element() -> frost_secp256k1::Error {
    frost_secp256k1::Error::GroupError(frost_secp256k1::GroupError::MalformedElement)
}
