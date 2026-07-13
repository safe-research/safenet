use super::{State, Transition};
use crate::{
    bindings::Coordinator,
    service::{Action, Effect},
};
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

    /// Links a committed nonce tree to its assigned onchain chunk. This can
    /// happen regardless of the current rollover/signing state. Only this
    /// validator's own commitment is linked; other participants' commitments
    /// are for their own local secret stores.
    pub(super) fn handle_preprocess(
        &self,
        state: State,
        event: &Coordinator::Preprocess,
    ) -> (State, Commands<State, Self>) {
        if event.participant != self.account {
            return (state, Vec::new());
        }

        tracing::debug!(
            group_id = %event.gid,
            chunk = event.chunk,
            root = %event.commitment,
            "linking nonce tree to onchain chunk"
        );
        (
            state,
            vec![Command::Effect(Effect::LinkNonceTree {
                group_id: event.gid,
                chunk: event.chunk,
                root: event.commitment,
            })],
        )
    }
}
