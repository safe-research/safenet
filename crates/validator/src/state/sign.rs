use super::{Packet, SigningState, State, Transition};
use crate::{
    bindings::{Coordinator, Oracle, SignNonces},
    service::{Action, Effect},
};
use alloy::primitives::{Address, B256};
use safenet_core::state::{Command, Commands};
use std::collections::BTreeMap;

impl Transition {
    /// Handles a validator's own request to sign a packet.
    pub(super) fn handle_sign(
        &self,
        mut state: State,
        block: u64,
        event: &Coordinator::Sign,
    ) -> (State, Commands<State, Self>) {
        let mut commands = Vec::new();
        match state.signing.remove(&event.message) {
            Some(SigningState::WaitingToDecline { deadline, .. }) => {
                commands.push(Command::Action(Action::SignDecline {
                    signature_id: event.sid,
                    expires_at: deadline,
                }));
            }
            Some(SigningState::WaitingForRequest {
                key_share,
                packet,
                signers,
                ..
            }) => match packet {
                Packet::OracleTransaction { oracle, .. } => {
                    let deadline = Some(block.saturating_add(self.config.oracle_timeout.get()));
                    state.signing.insert(
                        event.message,
                        SigningState::WaitingForOracle {
                            key_share,
                            oracle,
                            group_id: event.gid,
                            signature_id: event.sid,
                            sequence: event.sequence,
                            packet,
                            signers,
                            deadline,
                        },
                    );
                    state
                        .signature_id_to_message
                        .insert(event.sid, event.message);
                }
                Packet::Transaction { .. } => {
                    let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));
                    state.signing.insert(
                        event.message,
                        SigningState::CollectNonceCommitments {
                            key_share,
                            signature_id: event.sid,
                            revealed: BTreeMap::new(),
                            packet,
                            signers,
                            deadline,
                        },
                    );
                    state
                        .signature_id_to_message
                        .insert(event.sid, event.message);
                    commands.push(Command::Effect(Effect::RevealNonceCommitments {
                        group_id: event.gid,
                        signature_id: event.sid,
                        message: event.message,
                        sequence: event.sequence,
                    }));
                }
            },
            Some(other) => {
                tracing::warn!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "unexpected sign event for message",
                );
                state.signing.insert(event.message, other);
            }
            None => {
                tracing::debug!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "not participating in message signing ceremony",
                );
            }
        }

        (state, commands)
    }

    /// Publishes this validator's revealed nonce commitment once the
    /// [`Effect::RevealNonceCommitments`] effect has produced it, entering
    /// [`SigningState::CollectNonceCommitments`]'s collection round.
    pub(super) fn handle_nonce_commitments(
        &self,
        state: State,
        signature_id: B256,
        message: B256,
        nonces: SignNonces,
        proof: Vec<B256>,
    ) -> (State, Commands<State, Self>) {
        let deadline = match state.signing.get(&message) {
            Some(SigningState::CollectNonceCommitments {
                signature_id: sid,
                deadline,
                ..
            }) if *sid == signature_id => *deadline,
            _ => return (state, Vec::new()),
        };

        (
            state,
            vec![Command::Action(Action::RevealNonceCommitments {
                signature_id,
                nonces,
                proof,
                expires_at: deadline,
            })],
        )
    }

    /// Resolves an oracle-backed signing round once its result lands:
    /// approved, this validator reveals its nonce commitment (as in
    /// [`handle_sign`](Self::handle_sign)'s live-request case); rejected, the
    /// session is simply dropped. A result for anything other than a tracked
    /// [`SigningState::WaitingForOracle`] round is ignored, as is one from an
    /// oracle contract other than the one the packet named.
    pub(super) fn handle_oracle_result(
        &self,
        mut state: State,
        block: u64,
        oracle: Address,
        event: &Oracle::OracleResult,
    ) -> (State, Commands<State, Self>) {
        match state.signing.remove(&event.requestId) {
            Some(SigningState::WaitingForOracle {
                key_share,
                oracle: expected,
                signature_id,
                packet,
                signers,
                group_id,
                sequence,
                ..
            }) if expected == oracle && event.approved => {
                let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));
                state.signing.insert(
                    event.requestId,
                    SigningState::CollectNonceCommitments {
                        key_share,
                        signature_id,
                        revealed: BTreeMap::new(),
                        packet,
                        signers,
                        deadline,
                    },
                );

                (
                    state,
                    vec![Command::Effect(Effect::RevealNonceCommitments {
                        group_id,
                        signature_id,
                        message: event.requestId,
                        sequence,
                    })],
                )
            }
            Some(SigningState::WaitingForOracle {
                signature_id,
                oracle: expected,
                ..
            }) if expected == oracle && !event.approved => {
                // Rejected: drop the session, along with the signature id
                // index entry eagerly set when the round was opened.
                state.signature_id_to_message.remove(&signature_id);
                (state, Vec::new())
            }
            Some(other) => {
                tracing::warn!(
                    request_id = %event.requestId,
                    "unexpected oracle result for request",
                );
                state.signing.insert(event.requestId, other);
                (state, Vec::new())
            }
            None => (state, Vec::new()),
        }
    }
}
