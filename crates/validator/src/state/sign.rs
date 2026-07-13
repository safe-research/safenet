use super::{Packet, SigningState, State, Transition};
use crate::{
    bindings::{Coordinator, SignNonces},
    service::{Action, Effect},
};
use alloy::primitives::B256;
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
}
