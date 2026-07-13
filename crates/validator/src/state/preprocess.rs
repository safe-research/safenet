use super::{State, Transition};
use crate::service::Action;
use alloy::primitives::B256;
use safenet_core::state::{Command, Commands};

impl Transition {
    /// Publishes this validator's freshly sampled nonce tree commitment once
    /// the [`Effect::NonceTree`] effect has produced it.
    pub(super) fn handle_nonce_tree(
        &self,
        state: State,
        group_id: B256,
        nonces_commitment: B256,
    ) -> (State, Commands<State, Self>) {
        (
            state,
            vec![Command::Action(Action::Preprocess {
                group_id,
                nonces_commitment,
            })],
        )
    }
}
