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
        }
    }
}
