use super::{KeyGenConfirmation, KeyGenParticipation, RolloverState, State, Transition};
use crate::{
    bindings::Coordinator,
    consensus::epoch::EpochId,
    service::{Action, Effect},
};
use alloy::primitives::B256;
use safenet_core::state::{Command, Commands};
use std::collections::BTreeMap;

impl Transition {
    /// Publishes this validator's freshly sampled nonce tree commitment once
    /// the [`Effect::NonceTree`] effect has produced it.
    pub(super) fn handle_nonce_tree(
        &self,
        state: State,
        group_id: B256,
        nonces_commitment: B256,
    ) -> (State, Commands<State, Self>) {
        if !state
            .epochs
            .values()
            .any(|epoch| epoch.group.id() == group_id)
        {
            tracing::debug!(%group_id, "ignoring nonce tree resume for an untracked group");
            return (state, Vec::new());
        }

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

    /// Reaps epochs no longer needed by a signing ceremony and reconciles all
    /// process-local and persisted group secrets with the resulting state.
    pub(super) fn handle_group_reconciliation(
        &self,
        mut state: State,
    ) -> (State, Commands<State, Self>) {
        // Reap old participating epochs for which there are no more signing
        // ceremonies. This runs linearly through the entire signing state, but
        // only once per block.
        let oldest_epoch = state
            .signing
            .values()
            .map(|signing| signing.packet().epoch())
            .fold(state.active_epoch, EpochId::min);
        state.epochs = state.epochs.split_off(&oldest_epoch);

        let mut groups = state
            .epochs
            .values()
            .map(|epoch| (epoch.group.id(), Some(epoch.key_share.clone())))
            .collect::<BTreeMap<_, _>>();

        // Retain an in-progress DKG only while this validator participates in
        // it. `None` preserves any persisted material across the pre-key-share
        // phases without starting a nonce generator.
        let rollover_group = match &state.rollover {
            // In case we are in key generation state where we have already
            // built a key share.
            RolloverState::CollectingConfirmations {
                group,
                status: KeyGenConfirmation::Confirmed(key_share),
                ..
            }
            | RolloverState::SigningRollover {
                group,
                key_share: Some(key_share),
                ..
            } => Some((group.id(), Some(key_share.clone()))),
            // In case we are in a key generation state while we are still
            // building our secret key share.
            RolloverState::WaitingForSetup { group, .. }
            | RolloverState::CollectingCommitments {
                group,
                secrets: Some(_),
                ..
            }
            | RolloverState::CollectingShares {
                group,
                participation: KeyGenParticipation::Participating(_),
                ..
            }
            | RolloverState::CollectingConfirmations {
                group,
                participation: KeyGenParticipation::Participating(_),
                ..
            } => Some((group.id(), None)),
            // Any other key generation state means that we do not want to keep
            // any secrets around for that group.
            _ => None,
        };
        groups.extend(rollover_group);

        (
            state,
            vec![Command::Effect(Effect::ReconcileGroupSecrets { groups })],
        )
    }
}
