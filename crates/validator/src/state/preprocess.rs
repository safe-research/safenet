use super::{NonceIndex, NonceState, State, Transition};
use crate::{
    bindings::Coordinator,
    frost::preprocess::{self, SEQUENCE_CHUNK_SIZE},
    service::Action,
};
use alloy::primitives::B256;
use safenet_core::state::{Command, Commands};

/// The remaining canonical nonce capacity below which another chunk is
/// requested.
const NONCE_TOPUP_THRESHOLD: u64 = 100;

impl NonceState {
    /// Resolves `sequence` to durable secret-store coordinates, but only when
    /// its chunk has been canonically linked.
    pub(super) fn nonce(&self, sequence: u64) -> Option<NonceIndex> {
        let (chunk, offset) = preprocess::decode_sequence(sequence);
        self.chunks
            .get(&chunk)
            .copied()
            .flatten()
            .map(|root| NonceIndex { root, offset })
    }

    /// Advances the observed sequence and forgets reservations for chunks the
    /// chain has completely passed without registering.
    pub(super) fn observe(&mut self, sequence: u64) {
        self.next_sequence = sequence.saturating_add(1);
        let (next_chunk, _) = preprocess::decode_sequence(self.next_sequence);
        self.chunks
            .retain(|chunk, root| root.is_some() || *chunk >= next_chunk);
    }

    /// Reserves the next chunk when the canonical supply is below the low
    /// watermark. A pending reservation suppresses further requests until it
    /// is linked or the chain advances past it.
    pub(super) fn reserve_chunk(&mut self) -> Option<u64> {
        if self.chunks.values().any(Option::is_none) || self.available() >= NONCE_TOPUP_THRESHOLD {
            return None;
        }

        let chunk = self.expected_chunk()?;
        self.chunks.insert(chunk, None);
        Some(chunk)
    }

    /// Records an onchain assignment and clears the single pending
    /// reservation whose actual chunk has now been decided by the contract.
    pub(super) fn link(&mut self, chunk: u64, root: B256) {
        self.chunks.retain(|_, root| root.is_some());
        self.chunks.insert(chunk, Some(root));
    }

    /// Re-establishes the state reservation for a generation effect that
    /// survived a rollback of the state that originally requested it.
    fn ensure_reservation(&mut self) {
        if !self.chunks.values().any(Option::is_none)
            && let Some(chunk) = self.expected_chunk()
        {
            self.chunks.insert(chunk, None);
        }
    }

    /// Returns the chunk the onchain contract is expected to assign to the
    /// next commitment.
    fn expected_chunk(&self) -> Option<u64> {
        let (sequence_chunk, _) = preprocess::decode_sequence(self.next_sequence);
        match self.chunks.last_key_value() {
            Some((chunk, _)) => chunk.checked_add(1).map(|chunk| sequence_chunk.max(chunk)),
            None => Some(sequence_chunk),
        }
    }

    /// Counts canonical and pending nonce capacity from `next_sequence`.
    fn available(&self) -> u64 {
        let (sequence_chunk, offset) = preprocess::decode_sequence(self.next_sequence);
        self.chunks
            .range(sequence_chunk..)
            .map(|(chunk, _)| {
                if *chunk == sequence_chunk {
                    SEQUENCE_CHUNK_SIZE.saturating_sub(offset)
                } else {
                    SEQUENCE_CHUNK_SIZE
                }
            })
            .sum()
    }
}

impl Transition {
    /// Publishes this validator's freshly sampled nonce tree commitment once
    /// the [`Effect::NonceTree`] effect has produced it.
    pub(super) fn handle_nonce_tree(
        &self,
        mut state: State,
        group_id: B256,
        nonces_commitment: B256,
    ) -> (State, Commands<State, Self>) {
        let Some(epoch) = state
            .epochs
            .values_mut()
            .find(|epoch| epoch.group.id() == group_id)
        else {
            tracing::debug!(%group_id, "ignoring nonce tree resume for an untracked group");
            return (state, Vec::new());
        };
        epoch.nonces.ensure_reservation();

        (
            state,
            vec![Command::Action(Action::Preprocess {
                group_id,
                nonces_commitment,
            })],
        )
    }

    /// Records this validator's committed nonce tree at its canonical onchain
    /// chunk. Other participants' roots are irrelevant to local state.
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
            return (state, Vec::new());
        };

        epoch.nonces.link(event.chunk, event.commitment);
        tracing::debug!(
            group_id = %event.gid,
            chunk = event.chunk,
            root = %event.commitment,
            "recorded canonical nonce tree assignment"
        );
        (state, Vec::new())
    }
}
