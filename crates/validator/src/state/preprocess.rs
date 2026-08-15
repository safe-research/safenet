use super::{
    KeyGenConfirmation, KeyGenParticipation, NonceState, RolloverState, State, Transition,
};
use crate::{
    bindings::Coordinator,
    consensus::epoch::EpochId,
    frost::preprocess::{self, SEQUENCE_CHUNK_SIZE},
    service::{Action, Effect},
};
use alloy::primitives::B256;
use safenet_core::state::{Command, Commands};
use std::collections::BTreeMap;

/// The remaining canonical nonce capacity below which another chunk is
/// requested.
const NONCE_TOPUP_THRESHOLD: u64 = 100;

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

    /// Links a committed nonce tree to its assigned onchain chunk for the
    /// current validator. This can happen regardless of the current rollover
    /// or signing state.
    pub(super) fn handle_preprocess(
        &self,
        mut state: State,
        event: &Coordinator::Preprocess,
    ) -> (State, Commands<State, Self>) {
        if event.participant != self.account {
            return (state, Vec::new());
        }

        let Some(epoch) = state
            .epochs
            .values_mut()
            .find(|epoch| epoch.group.id() == event.gid)
        else {
            tracing::warn!(group_id = %event.gid, "nonce preprocess event for unknown epoch");
            return (state, Vec::new());
        };

        tracing::debug!(
            group_id = %event.gid,
            chunk = event.chunk,
            root = %event.commitment,
            "linking nonce tree to onchain chunk"
        );
        epoch.nonces.link(event.chunk, event.commitment);

        (
            state,
            vec![Command::Effect(Effect::LinkNonceTree {
                group_id: event.gid,
                chunk: event.chunk,
                root: event.commitment,
            })],
        )
    }

    /// Requests another nonce tree for the active epoch when its canonical
    /// supply falls below the top-up threshold.
    pub(super) fn handle_nonce_topup(&self, mut state: State) -> (State, Commands<State, Self>) {
        let active_epoch = state.active_epoch;
        let Some(epoch) = state.epochs.get_mut(&active_epoch) else {
            return (state, Vec::new());
        };

        if epoch.nonces.available() >= NONCE_TOPUP_THRESHOLD {
            return (state, Vec::new());
        }

        let group_id = epoch.group.id();
        let Some(chunk) = epoch.nonces.reserve_chunk() else {
            tracing::warn!(?active_epoch, %group_id, "nonce chunk sequence exhausted; cannot top up");
            return (state, Vec::new());
        };

        tracing::debug!(?active_epoch, %group_id, chunk, "requesting nonce tree top-up");
        let key_share = epoch.key_share.clone();

        (
            state,
            vec![Command::Effect(Effect::NonceTree {
                group_id,
                key_share,
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
        groups.extend(match &state.rollover {
            // Already have a key share, either because our secret shares were
            // all verified or the group's rollover proposal is being signed.
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
            // Still building our secret key share.
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
        });

        (
            state,
            vec![Command::Effect(Effect::ReconcileGroupSecrets { groups })],
        )
    }
}

impl NonceState {
    /// Advances the observed sequence and forgets past roots for nonce chunks
    /// that can no longer used for signing.
    pub(super) fn observe(&mut self, sequence: u64) {
        self.next_sequence = sequence.saturating_add(1);
        let (chunk, _) = preprocess::decode_sequence(self.next_sequence);
        self.chunks = self.chunks.split_off(&chunk);
    }

    /// Reserves the next chunk, marking it as pending in the nonce state.
    ///
    /// Returns `None` if all nonce chunks have been exhausted.
    pub(super) fn reserve_chunk(&mut self) -> Option<u64> {
        let chunk = self.expected_chunk()?;
        self.chunks.insert(chunk, None);
        Some(chunk)
    }

    /// Records an onchain assignment.
    pub(super) fn link(&mut self, chunk: u64, root: B256) {
        self.chunks.insert(chunk, Some(root));
    }

    /// Returns the chunk the onchain contract is expected to assign to the
    /// next commitment.
    ///
    /// Returns `None` in case we've reached the very last chunk.
    fn expected_chunk(&self) -> Option<u64> {
        let (chunk, _) = preprocess::decode_sequence(self.next_sequence);
        match self.chunks.last_key_value() {
            Some((last, _)) => last.checked_add(1).map(|next| next.max(chunk)),
            None => Some(chunk),
        }
    }

    /// Counts canonical and pending nonce capacity from `next_sequence`.
    fn available(&self) -> u64 {
        let (chunk, offset) = preprocess::decode_sequence(self.next_sequence);
        self.chunks
            .range(chunk..)
            .map(|(key, _)| {
                if *key == chunk {
                    SEQUENCE_CHUNK_SIZE.saturating_sub(offset)
                } else {
                    SEQUENCE_CHUNK_SIZE
                }
            })
            .sum()
    }
}
