pub mod storage;

use std::collections::BTreeMap;

pub use self::storage::Storage;
use crate::{
    actions::Action,
    bindings::{self, Coordinator},
    frost::{keygen, participants, secret::EncryptionKey},
};
use alloy::primitives::{Address, B256};
use frost_secp256k1::keys::dkg;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub own_address: Address,
    pub participants: Vec<Address>,
    pub genesis_salt: Option<B256>,
    pub blocks_per_epoch: u64,
}

#[derive(Serialize, Deserialize)]
pub enum Phase {
    WaitingForGenesis {
        genesis_group_id: B256,
    },
    WaitingForRollover,
    CollectingCommitments {
        gid: B256,
        encryption_key: EncryptionKey,
        secret_package: dkg::round1::SecretPackage,
        commitments: BTreeMap<Address, bindings::KeyGenCommitment>,
    },
    CollectingShares {
        gid: B256,
        encryption_key: EncryptionKey,
        secret_package: dkg::round2::SecretPackage,
        commitments: BTreeMap<Address, bindings::KeyGenCommitment>,
        shares: BTreeMap<Address, bindings::KeyGenSecretShare>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct ValidatorState {
    pub consensus_config: ConsensusConfig,
    pub last_seen_block: Option<u64>,
    pub phase: Phase,
}

impl ValidatorState {
    pub fn new(active_epoch: u64, consensus_config: ConsensusConfig) -> Self {
        let genesis_group_id = participants::calc_genesis_group_id(
            &consensus_config.participants,
            consensus_config.genesis_salt,
        );
        Self {
            consensus_config,
            last_seen_block: None,
            phase: if active_epoch == 0 {
                Phase::WaitingForGenesis { genesis_group_id }
            } else {
                Phase::WaitingForRollover
            },
        }
    }

    pub fn on_block(&mut self, block_number: u64) -> Vec<Action> {
        self.last_seen_block = Some(block_number);
        vec![]
    }

    pub fn on_consensus_event(
        &mut self,
        event: crate::bindings::Consensus::ConsensusEvents,
    ) -> Vec<Action> {
        tracing::info!(?event, "consensus event");
        vec![]
    }

    pub fn on_coordinator_event(&mut self, event: Coordinator::CoordinatorEvents) -> Vec<Action> {
        tracing::info!(?event, "coordinator event");
        match event {
            Coordinator::CoordinatorEvents::KeyGen(e) => self.on_keygen(e),
            Coordinator::CoordinatorEvents::KeyGenCommitted(e) => self.on_keygen_committed(e),
            Coordinator::CoordinatorEvents::KeyGenSecretShared(e) => {
                self.on_keygen_secret_shared(e)
            }
            _ => vec![],
        }
    }

    fn on_keygen(&mut self, event: Coordinator::KeyGen) -> Vec<Action> {
        let Phase::WaitingForGenesis { genesis_group_id } = &self.phase else {
            return vec![];
        };
        if event.gid != *genesis_group_id {
            return vec![];
        }

        let gid = event.gid;
        let config = &self.consensus_config;

        let Some(poap) =
            participants::generate_participant_proof(&config.participants, config.own_address)
        else {
            tracing::info!(%gid, "not a participant in genesis keygen");
            self.phase = Phase::WaitingForRollover;
            return vec![];
        };

        tracing::info!(%gid, "genesis keygen triggered, publishing commitment");
        let round1 = match keygen::generate_round1(config.own_address, event.count, event.threshold)
        {
            Ok(result) => result,
            Err(err) => {
                tracing::error!(%err, "DKG round 1 failed");
                return vec![];
            }
        };

        self.phase = Phase::CollectingCommitments {
            gid,
            encryption_key: round1.encryption_key,
            secret_package: round1.secret_package,
            commitments: BTreeMap::new(),
        };
        vec![Action::KeyGenAndCommit {
            participants: event.participants,
            count: event.count,
            threshold: event.threshold,
            context: event.context,
            poap,
            commitment: round1.commitment,
        }]
    }

    fn on_keygen_committed(&mut self, event: Coordinator::KeyGenCommitted) -> Vec<Action> {
        let Phase::CollectingCommitments {
            gid,
            encryption_key,
            secret_package,
            commitments,
        } = &mut self.phase
        else {
            return vec![];
        };

        if event.gid != *gid {
            return vec![];
        }

        commitments.insert(event.participant, event.commitment);

        if commitments.len() < self.consensus_config.participants.len() {
            return vec![];
        }

        // TODO(nlordell): we should modify this code to move out the old fields
        // from the `phase` instead of cloning them.
        tracing::info!("all participants committed, transitioning to collecting shares");
        let round2 = match keygen::generate_round2(encryption_key, secret_package, commitments) {
            Ok(round2) => round2,
            Err(err) => {
                tracing::error!(%err, "DKG round 2 failed");
                return vec![];
            }
        };

        self.phase = Phase::CollectingShares {
            gid: event.gid,
            encryption_key: encryption_key.clone(),
            secret_package: round2.secret_package,
            commitments: commitments.clone(),
            shares: BTreeMap::new(),
        };
        vec![Action::KeyGenSecretShare {
            gid: event.gid,
            share: round2.share,
        }]
    }

    fn on_keygen_secret_shared(&mut self, event: Coordinator::KeyGenSecretShared) -> Vec<Action> {
        let Phase::CollectingShares {
            gid,
            encryption_key,
            secret_package,
            commitments,
            shares,
        } = &mut self.phase
        else {
            return vec![];
        };

        if event.gid != *gid || !event.shared {
            return vec![];
        }

        shares.insert(event.participant, event.share);

        if shares.len() < self.consensus_config.participants.len() {
            return vec![];
        }

        tracing::info!("all secret shares received");
        let round3 =
            match keygen::generate_round3(encryption_key, secret_package, commitments, shares) {
                Ok(round2) => round2,
                Err(err) => {
                    tracing::error!(%err, "DKG round 2 failed");
                    return vec![];
                }
            };
    }
}
