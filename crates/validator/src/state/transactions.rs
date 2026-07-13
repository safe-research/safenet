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
        let epoch = EpochId::from_raw(event.epoch);
        let Some(participating_epoch) = state.epochs.get(&epoch) else {
            return (state, Vec::new());
        };

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

            let signers = participating_epoch.group.participants().clone();
            let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));

            let signing_state = if checks::check_transaction(&event.transaction) {
                SigningState::WaitingForRequest {
                    key_share: participating_epoch.key_share.clone(),
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

    /// Clears a completed signing session once its attestation lands
    /// onchain, keyed by the same proposal hash used to open it:
    /// [`SigningState::WaitingForRequest`] verifies the full transaction,
    /// while the attestation only carries its
    /// [`safe_tx_hash`](crate::consensus::hashing::safe_tx_hash), so the
    /// proposal is re-hashed directly from the event rather than the packet.
    pub(super) fn handle_transaction_attested(
        &self,
        state: State,
        event: &Consensus::TransactionAttested,
    ) -> (State, Commands<State, Self>) {
        let epoch = EpochId::from_raw(event.epoch);
        let message = self
            .consensus
            .transaction_proposal_hash(epoch, event.safeTxHash);
        self.handle_sign_attested(state, event.signatureId, message)
    }

    /// Verifies a proposed oracle-backed Safe transaction against the
    /// epoch's resolved group, opening a [`SigningState::WaitingForRequest`]
    /// signing session for it. Unlike a plain [`Consensus::TransactionProposed`],
    /// the transaction itself is not checked against Safenet policy here -
    /// the oracle vouches for it, and its result is checked once attested -
    /// only the oracle's own identity is verified against the configured
    /// allow-list. A transaction proposed for an unknown or unresolved epoch,
    /// one whose group this validator is not part of, or from a disallowed
    /// oracle, is ignored.
    pub(super) fn handle_oracle_transaction_proposed(
        &self,
        mut state: State,
        block: u64,
        event: &Consensus::OracleTransactionProposed,
    ) -> (State, Commands<State, Self>) {
        let epoch = EpochId::from_raw(event.epoch);
        let Some(participating_epoch) = state
            .epochs
            .get(&epoch)
            .filter(|_| self.config.oracles.contains(&event.oracle))
        else {
            return (state, Vec::new());
        };

        let message =
            self.consensus
                .oracle_transaction_packet_hash(epoch, event.oracle, &event.transaction);

        // Prevent duplicate ongoing transaction proposals. This is to prevent
        // malicious parties from blocking transaction attestations from ever
        // being produced by resetting the signing state of honest validators.
        if let btree_map::Entry::Vacant(signing) = state.signing.entry(message) {
            let packet = Packet::OracleTransaction {
                epoch,
                oracle: event.oracle,
                transaction: event.transaction.clone(),
            };
            let signers = participating_epoch.group.participants().clone();
            let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));

            signing.insert(SigningState::WaitingForRequest {
                key_share: participating_epoch.key_share.clone(),
                packet,
                signers,
                deadline,
            });
        } else {
            tracing::warn!(%message, "ignoring duplicate oracle transaction proposal");
        }

        (state, Vec::new())
    }

    /// Clears a completed oracle-backed signing session once its attestation
    /// lands onchain.
    pub(super) fn handle_oracle_transaction_attested(
        &self,
        state: State,
        event: &Consensus::OracleTransactionAttested,
    ) -> (State, Commands<State, Self>) {
        let epoch = EpochId::from_raw(event.epoch);
        let message =
            self.consensus
                .oracle_transaction_proposal_hash(epoch, event.oracle, event.safeTxHash);
        self.handle_sign_attested(state, event.signatureId, message)
    }
}
