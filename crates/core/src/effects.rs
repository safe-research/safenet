//! Effect handling for state transitions.

use std::{convert::Infallible, future::Future};

/// An effect handler that can be used for handling effects for a state
/// transition.
pub trait EffectHandler<Effect, Resume>: Send + Sync + 'static {
    /// Performs an effect and returns its result to resume a state machine.
    ///
    /// Note that this method explicitly does not fail, and is expected to return
    /// some result that indicates an error so the state machine can handle the
    /// effect error internally.
    ///
    /// The same effect may be performed more than once for the same chain
    /// message. For consumptive resources such as pre-committed nonces, handlers
    /// should encode outcomes like "already used" in `Resume`; state transitions
    /// remain pure because they consume only the resume value.
    fn perform_effect(&self, effect: Effect) -> impl Future<Output = Resume> + Send;
}

/// An effect handler for pure state machines without any side-effects.
pub struct Pure;

impl<Resume> EffectHandler<Infallible, Resume> for Pure {
    async fn perform_effect(&self, effect: Infallible) -> Resume {
        match effect {}
    }
}
