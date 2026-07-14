//! Validator actions and their encoding into transactions.

use crate::{
    bindings::{self, Consensus, Coordinator},
    consensus::epoch::EpochId,
    merkle::MerkleRoot,
};
use alloy::{
    primitives::{Address, B256, U256},
    sol_types::SolCall,
};
use safenet_core::{driver::ActionEncoder, tx::Transaction};
use std::num::NonZeroU64;

/// An onchain action the validator emits during a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(dead_code)]
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
        callback: Option<bindings::Callback>,
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
    /// An action to reveal this validator's nonce commitment for a signing
    /// round.
    RevealNonceCommitments {
        signature_id: B256,
        nonces: bindings::SignNonces,
        proof: Vec<B256>,
        expires_at: u64,
    },
    /// An action to decline participation in a signing round for a packet
    /// that failed verification.
    SignDecline { signature_id: B256, expires_at: u64 },
    /// An action to publish this validator's signature share, along with the
    /// callback invoked once the group's signature completes.
    SignShare {
        signature_id: B256,
        selection: bindings::SignSelection,
        share: bindings::SignatureShare,
        proof: Vec<B256>,
        callback: bindings::Callback,
        expires_at: u64,
    },
    /// An action to open a signing round for a verified packet, retried by
    /// whichever participant is responsible after a timeout.
    Sign {
        group_id: B256,
        message: B256,
        expires_at: u64,
    },
    /// A fallback action to submit a completed transaction attestation
    /// directly, when the automatic `signShareWithCallback` submission did
    /// not land in time.
    AttestTransaction {
        epoch: EpochId,
        chain_id: U256,
        safe: Address,
        safe_tx_struct_hash: B256,
        signature_id: B256,
        expires_at: u64,
    },
    /// A fallback action to submit a completed oracle-backed transaction
    /// attestation directly, when the automatic `signShareWithCallback`
    /// submission did not land in time.
    AttestOracleTransaction {
        epoch: EpochId,
        oracle: Address,
        chain_id: U256,
        safe: Address,
        safe_tx_struct_hash: B256,
        signature_id: B256,
        expires_at: u64,
    },
    /// A fallback action to stage a completed epoch rollover directly, when
    /// the automatic `signShareWithCallback` submission did not land in
    /// time.
    StageEpoch {
        proposed_epoch: NonZeroU64,
        rollover_block: u64,
        group_id: B256,
        signature_id: B256,
        expires_at: u64,
    },
}

/// Encodes [`Action`]s into the transactions the queue submits.
pub struct Encoder {
    pub coordinator: Address,
    pub consensus: Address,
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
                callback,
                expires_at,
            } => {
                let (data, gas) = match callback {
                    Some(callback) => (
                        Coordinator::keyGenConfirmWithCallbackCall {
                            gid: group_id,
                            callback,
                        }
                        .abi_encode(),
                        300_000,
                    ),
                    None => (
                        Coordinator::keyGenConfirmCall { gid: group_id }.abi_encode(),
                        200_000,
                    ),
                };
                (
                    Transaction {
                        to: self.coordinator,
                        value: U256::ZERO,
                        data: data.into(),
                        gas,
                    },
                    expires_at,
                )
            }
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
                    gas: 300_000,
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
            Action::RevealNonceCommitments {
                signature_id,
                nonces,
                proof,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::signRevealNoncesCall {
                        sid: signature_id,
                        nonces,
                        proof,
                    }
                    .abi_encode()
                    .into(),
                    gas: 250_000,
                },
                Some(expires_at),
            ),
            Action::SignDecline {
                signature_id,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::signDeclineCall { sid: signature_id }
                        .abi_encode()
                        .into(),
                    gas: 150_000,
                },
                Some(expires_at),
            ),
            Action::SignShare {
                signature_id,
                selection,
                share,
                proof,
                callback,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::signShareWithCallbackCall {
                        sid: signature_id,
                        selection,
                        share,
                        proof,
                        callback,
                    }
                    .abi_encode()
                    .into(),
                    gas: 400_000,
                },
                Some(expires_at),
            ),
            Action::Sign {
                group_id,
                message,
                expires_at,
            } => (
                Transaction {
                    to: self.coordinator,
                    value: U256::ZERO,
                    data: Coordinator::signCall {
                        gid: group_id,
                        message,
                    }
                    .abi_encode()
                    .into(),
                    gas: 150_000,
                },
                Some(expires_at),
            ),
            Action::AttestTransaction {
                epoch,
                chain_id,
                safe,
                safe_tx_struct_hash,
                signature_id,
                expires_at,
            } => (
                Transaction {
                    to: self.consensus,
                    value: U256::ZERO,
                    data: Consensus::attestTransactionCall {
                        epoch: epoch.raw_value(),
                        chainId: chain_id,
                        safe,
                        safeTxStructHash: safe_tx_struct_hash,
                        signatureId: signature_id,
                    }
                    .abi_encode()
                    .into(),
                    gas: 250_000,
                },
                Some(expires_at),
            ),
            Action::AttestOracleTransaction {
                epoch,
                oracle,
                chain_id,
                safe,
                safe_tx_struct_hash,
                signature_id,
                expires_at,
            } => (
                Transaction {
                    to: self.consensus,
                    value: U256::ZERO,
                    data: Consensus::attestOracleTransactionCall {
                        epoch: epoch.raw_value(),
                        oracle,
                        chainId: chain_id,
                        safe,
                        safeTxStructHash: safe_tx_struct_hash,
                        signatureId: signature_id,
                    }
                    .abi_encode()
                    .into(),
                    gas: 250_000,
                },
                Some(expires_at),
            ),
            Action::StageEpoch {
                proposed_epoch,
                rollover_block,
                group_id,
                signature_id,
                expires_at,
            } => (
                Transaction {
                    to: self.consensus,
                    value: U256::ZERO,
                    data: Consensus::stageEpochCall {
                        proposedEpoch: proposed_epoch.get(),
                        rolloverBlock: rollover_block,
                        groupId: group_id,
                        signatureId: signature_id,
                    }
                    .abi_encode()
                    .into(),
                    gas: 250_000,
                },
                Some(expires_at),
            ),
        }
    }
}
