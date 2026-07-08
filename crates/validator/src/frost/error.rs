/// A Safenet FROST error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The underlying FROST ciphersuite error.
    #[error(transparent)]
    Frost(#[from] frost_secp256k1::Error),
}

impl Error {
    /// An error indicating a malformed scalar.
    pub const fn malformed_scalar() -> Self {
        Self::Frost(frost_core::Error::FieldError(
            frost_core::FieldError::MalformedScalar,
        ))
    }

    /// An error indicating a malformed group element (point).
    pub const fn malformed_element() -> Self {
        Self::Frost(frost_core::Error::GroupError(
            frost_core::GroupError::MalformedElement,
        ))
    }
}
