pub mod storage;

pub use self::storage::Storage;
use crate::{
    actions::Action,
    bindings::{Consensus, Coordinator},
    frost::participants::calc_genesis_group_id,
};
use alloy::primitives::{Address, B256};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub participants: Vec<Address>,
    pub genesis_salt: Option<B256>,
    pub blocks_per_epoch: u64,
}

#[derive(Serialize, Deserialize)]
pub enum Phase {
    WaitingForGenesis { genesis_group_id: B256 },
    WaitingForRollover,
    CollectingCommitments { gid: B256 },
}

#[derive(Serialize, Deserialize)]
pub struct ValidatorState {
    pub last_seen_block: Option<u64>,
    pub phase: Phase,
    pub consensus_config: ConsensusConfig,
}

impl ValidatorState {
    pub fn new(active_epoch: u64, consensus_config: ConsensusConfig) -> Self {
        let genesis_group_id = calc_genesis_group_id(
            &consensus_config.participants,
            consensus_config.genesis_salt,
        );
        Self {
            last_seen_block: None,
            phase: if active_epoch == 0 {
                Phase::WaitingForGenesis { genesis_group_id }
            } else {
                Phase::WaitingForRollover
            },
            consensus_config,
        }
    }

    pub fn on_block(&mut self, block_number: u64) -> Vec<Action> {
        self.last_seen_block = Some(block_number);
        vec![]
    }

    pub fn on_consensus_event(&mut self, event: Consensus::ConsensusEvents) -> Vec<Action> {
        tracing::info!(?event, "consensus event");
        vec![]
    }

    pub fn on_coordinator_event(&mut self, event: Coordinator::CoordinatorEvents) -> Vec<Action> {
        tracing::info!(?event, "coordinator event");
        match event {
            Coordinator::CoordinatorEvents::KeyGen(e) => self.on_keygen(e),
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
        tracing::info!(%gid, "genesis key generation started, generating commitment");
        self.phase = Phase::CollectingCommitments { gid };
        vec![Action::KeyGenAndCommit { gid }]
    }
}
