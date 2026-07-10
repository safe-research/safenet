//! The validator effect system and its handler.

use safenet_core::state::EffectHandler;

/// An impure operation the state transition asks the handler to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {}

/// The result of performing an [`Effect`], resumed into the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resume {}

/// Performs the validator's [`Effect`]s, resuming with a [`Resume`].
pub struct Handler;

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&mut self, effect: Effect) -> Resume {
        match effect {}
    }
}
