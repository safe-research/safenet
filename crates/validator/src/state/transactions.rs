use std::collections::btree_map;

use super::{Packet, SigningState, State, Transition};
use crate::{bindings::Consensus, consensus::epoch::EpochId};
use safenet_core::state::Commands;

impl Transition {
    /// Verifies a proposed oracle-backed Safe transaction against the
    /// epoch's resolved group, opening a [`SigningState::WaitingForRequest`]
    /// signing session for it. The transaction itself is not checked against
    /// Safenet policy here - the oracle vouches for it, and its result is
    /// checked once attested - only the oracle's own identity is verified
    /// against the configured allow-list. A transaction proposed for an
    /// unknown or unresolved epoch, one whose group this validator is not
    /// part of, or from a disallowed oracle, is ignored.
    pub(super) fn handle_transaction_proposed(
        &self,
        mut state: State,
        block: u64,
        event: &Consensus::TransactionProposed,
    ) -> (State, Commands<State, Self>) {
        let epoch = EpochId::from_raw(event.epoch);
        let Some(participating_epoch) = state.epochs.get(&epoch) else {
            tracing::debug!(
                ?epoch,
                oracle = %event.oracle,
                safe_tx_hash = %event.safeTxHash,
                "ignoring oracle transaction proposal for non-participating epoch"
            );
            return (state, Vec::new());
        };
        if !self.config.oracles.contains(&event.oracle) {
            tracing::debug!(
                ?epoch,
                oracle = %event.oracle,
                safe_tx_hash = %event.safeTxHash,
                "ignoring transaction proposal from unknown oracle"
            );
            return (state, Vec::new());
        }

        let message = self.consensus.transaction_packet_hash(
            epoch,
            event.oracle,
            event.oracleData.clone(),
            &event.transaction,
        );

        // Prevent duplicate ongoing transaction proposals. This is to prevent
        // malicious parties from blocking transaction attestations from ever
        // being produced by resetting the signing state of honest validators.
        if let btree_map::Entry::Vacant(signing) = state.signing.entry(message) {
            let packet = Packet::Transaction {
                epoch,
                oracle: event.oracle,
                oracle_data: event.oracleData.clone(),
                transaction: Box::new(event.transaction.clone()),
            };
            let signers = participating_epoch.group.participants().clone();
            let deadline = block.saturating_add(self.config.signing_timeout.get());
            let group_id = participating_epoch.group.id();

            signing.insert(SigningState::WaitingForRequest {
                key_share: participating_epoch.key_share.clone(),
                group_id,
                responsible: None,
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
    pub(super) fn handle_transaction_attested(
        &self,
        state: State,
        event: &Consensus::TransactionAttested,
    ) -> (State, Commands<State, Self>) {
        let epoch = EpochId::from_raw(event.epoch);
        let message = self.consensus.transaction_proposal_hash(
            epoch,
            event.oracle,
            event.oracleDataHash,
            event.safeTxHash,
        );
        tracing::info!(
            ?epoch,
            safe_tx_hash = %event.safeTxHash,
            oracle = %event.oracle,
            signature_id = %event.signatureId,
            "oracle transaction attested"
        );
        self.handle_sign_attested(state, event.signatureId, message)
    }
}
