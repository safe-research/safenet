use std::collections::btree_map;

use super::{Packet, SigningState, State, Transition};
use crate::{
    bindings::Consensus,
    consensus::{checks, epoch::EpochId},
};
use safenet_core::state::Commands;

impl Transition {
    /// Verifies a proposed Safe transaction against the epoch's resolved
    /// group, opening a signing session for it: [`SigningState::WaitingForRequest`]
    /// if it passes Safenet policy, [`SigningState::WaitingToDecline`]
    /// otherwise. A transaction proposed for an unknown or unresolved epoch,
    /// or one whose group this validator is not part of, is ignored.
    pub(super) fn handle_transaction_proposed(
        &self,
        mut state: State,
        block: u64,
        event: &Consensus::TransactionProposed,
    ) -> (State, Commands<State, Self>) {
        let Some(group) = state
            .epoch_groups
            .get(&event.epoch)
            .filter(|group| group.participants().contains(&self.account))
        else {
            return (state, Vec::new());
        };

        let epoch = EpochId::from_raw(event.epoch);
        let message = self
            .consensus
            .transaction_packet_hash(epoch, &event.transaction);

        // Prevent duplicate ongoing transaction proposals. This is to prevent
        // malicious parties from blocking transaction attestations from ever
        // being produced by resetting the signing state of honest validators.
        if let btree_map::Entry::Vacant(signing) = state.signing.entry(message) {
            let packet = Packet::Transaction {
                epoch,
                transaction: event.transaction.clone(),
            };

            let signers = group.participants().clone();
            let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));

            let signing_state = if checks::check_transaction(&event.transaction) {
                SigningState::WaitingForRequest {
                    packet,
                    signers,
                    deadline,
                }
            } else {
                SigningState::WaitingToDecline { packet, deadline }
            };

            signing.insert(signing_state);
        } else {
            tracing::warn!(
                %message,
                "ignoring duplicate transaction proposal"
            )
        }

        (state, Vec::new())
    }
}
