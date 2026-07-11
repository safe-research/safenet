//! Validator actions and their encoding into transactions.

use crate::{
    bindings::{self, Coordinator},
    merkle::MerkleRoot,
};
use alloy::{
    primitives::{Address, B256, U256},
    sol_types::SolCall,
};
use safenet_core::{driver::ActionEncoder, tx::Transaction};

/// An onchain action the validator emits during a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// An action to publish key generation commitments onchain.
    KeyGenAndCommit {
        participants: MerkleRoot,
        count: u16,
        threshold: u16,
        context: B256,
        poap: Vec<B256>,
        commitment: bindings::KeyGenCommitment,
        expires_at: Option<u64>,
    },
    /// An action to publish a key generation secret share onchain.
    KeyGenSecretShare {
        group_id: B256,
        share: bindings::KeyGenSecretShare,
        expires_at: Option<u64>,
    },
    /// An action to complain about an invalid key generation secret share.
    KeyGenComplain {
        group_id: B256,
        accused: Address,
        expires_at: Option<u64>,
    },
    /// An action to confirm participation in a completed key generation.
    KeyGenConfirm {
        group_id: B256,
        expires_at: Option<u64>,
    },
    /// An action to reveal a unencrypted secret share, in response to a
    /// complaint raised against the validator.
    KeyGenComplaintResponse {
        group_id: B256,
        plaintiff: Address,
        secret_share: U256,
        expires_at: Option<u64>,
    },
    /// An action to perform the preprocessing step and register a freshly
    /// sampled nonce tree's commitments.
    Preprocess {
        group_id: B256,
        nonces_commitment: B256,
    },
}

/// Encodes [`Action`]s into the transactions the queue submits.
pub struct Encoder {
    pub coordinator: Address,
}

impl ActionEncoder<Action> for Encoder {
    fn encode_action(&self, action: Action) -> (Transaction, Option<u64>) {
        match action {
            Action::KeyGenAndCommit {
                participants,
                count,
                threshold,
                context,
                poap,
                commitment,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::keyGenAndCommitCall {
                        participants: participants.0,
                        count,
                        threshold,
                        context,
                        poap,
                        commitment,
                    }
                    .abi_encode()
                    .into(),
                    gas: 250_000,
                },
                expires_at,
            ),
            Action::KeyGenSecretShare {
                group_id,
                share,
                expires_at,
            } => {
                let gas = 250_000 + 25_000 * share.f.len() as u64;
                (
                    Transaction {
                        to: self.coordinator,
                        value: U256::ZERO,
                        data: Coordinator::keyGenSecretShareCall {
                            gid: group_id,
                            share,
                        }
                        .abi_encode()
                        .into(),
                        gas,
                    },
                    expires_at,
                )
            }
            Action::KeyGenComplain {
                group_id,
                accused,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::keyGenComplainCall {
                        gid: group_id,
                        accused,
                    }
                    .abi_encode()
                    .into(),
                    gas: 300_000,
                },
                expires_at,
            ),
            Action::KeyGenConfirm {
                group_id,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::keyGenConfirmCall { gid: group_id }
                        .abi_encode()
                        .into(),
                    gas: 200_000,
                },
                expires_at,
            ),
            Action::KeyGenComplaintResponse {
                group_id,
                plaintiff,
                secret_share,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::keyGenComplaintResponseCall {
                        gid: group_id,
                        plaintiff,
                        secretShare: secret_share,
                    }
                    .abi_encode()
                    .into(),
                    gas: 250_000,
                },
                expires_at,
            ),
            Action::Preprocess {
                group_id,
                nonces_commitment,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::preprocessCall {
                        gid: group_id,
                        commitment: nonces_commitment,
                    }
                    .abi_encode()
                    .into(),
                    gas: 250_000,
                },
                // Nonce registration doesn't carry an expiry - we cannot
                // reliably know for how long it is valuable.
                None,
            ),
        }
    }
}
