//! Validator actions and their encoding into transactions.

use safenet_core::{driver::ActionEncoder, tx::Transaction};

/// An onchain action the validator emits during a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {}

/// Encodes [`ValidatorAction`]s into the transactions the queue submits.
pub struct Encoder;

impl ActionEncoder<Action> for Encoder {
    fn encode_action(&self, action: Action) -> (Transaction, Option<u64>) {
        match action {}
    }
}
