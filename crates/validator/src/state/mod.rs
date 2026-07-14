//! The snapshotted validator state.

mod keygen;
mod preprocess;
mod sign;
mod transactions;

use crate::{
    bindings::{Consensus, Coordinator, Oracle, Point, SafeTransaction},
    config::ValidatorConfig,
    consensus::{
        epoch::EpochId,
        group::{Group, ParticipantSet},
        hashing::ConsensusDomain,
    },
    frost::{
        keygen::{
            GroupCommitments, KeyShare, PublicKeyShare, Secrets, SharingState, VerifiedCommitment,
            VerifiedShare,
        },
        sign::RevealedNonces,
    },
    merkle::MerkleRoot,
    service::{Action, Effect, Event, Resume},
};
use alloy::primitives::{Address, B256};
use safenet_core::state::{Commands, Message, StateTransition};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    sync::Arc,
};

/// The complete snapshotted validator state.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct State {
    /// The epoch-rollover / DKG state machine.
    rollover: RolloverState,
    /// The epoch whose group is currently active in consensus.
    active_epoch: EpochId,
    /// The epochs that the validator is participating in and have completed
    /// their key generation, keyed by epoch number and retained past
    /// `EpochStaged` so later handlers can look up a resolved group and the
    /// generated key share.
    epochs: BTreeMap<EpochId, Epoch>,
    /// The signing sessions tracked so far, keyed by the message hash the
    /// group signature attests to.
    signing: BTreeMap<B256, SigningState>,
    /// The message hash each pending signature id was requested over, so its
    /// signing session can be found once the signature is submitted onchain.
    signature_id_to_message: BTreeMap<B256, B256>,
    /// Things queued up to be pruned, along with the block at which they were
    /// queued. Pruning itself only happens once the entry is mature enough to
    /// be reorg-safe.
    to_prune: Vec<(u64, Prune)>,
}

impl State {
    /// Queues `prune` to be pruned, recording the current `block` so its
    /// maturity can later be determined.
    fn and_prune(mut self, block: u64, prune: Prune) -> Self {
        self.to_prune.push((block, prune));
        self
    }
}

/// A resolved epoch that the validator is participating in, retained past
/// `EpochStaged` to access resolved group and epoch key material.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Epoch {
    /// The resolved group.
    group: Group,
    /// This validator's key share.
    key_share: Arc<KeyShare>,
}

/// Something queued up to be pruned once mature.
#[derive(Clone, Debug, Deserialize, Serialize)]
enum Prune {
    /// A resolved group's keygen secrets.
    KeyGenSecrets {
        /// The resolved group.
        group_id: B256,
    },
    /// A retired epoch's registered nonce trees.
    GroupNonces {
        /// The retired group.
        group_id: B256,
    },
}

/// The epoch-rollover / DKG state machine. Each active variant carries the
/// group it is generating.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
enum RolloverState {
    /// Idle before the genesis group's DKG has been triggered.
    #[default]
    WaitingForGenesis,
    /// The rollover has halted in an unrecoverable way.
    ///
    /// This can either be an unrecoverable error during DKG or we have reached
    /// the heat death of the universe and there are no more epochs.
    Halted,
    /// This group's key generation completed and the group is staged in the
    /// consensus contract.
    EpochStaged {
        /// The epoch that was just staged.
        next_epoch: NonZeroU64,
    },
    /// The key generation for `next_epoch` was skipped because too few
    /// participants took part for the group to be safe.
    EpochSkipped {
        /// The epoch whose key generation was skipped.
        next_epoch: NonZeroU64,
    },
    /// This validator is participating in the group's key generation and is
    /// waiting for the [`Effect::KeyGenSetup`] effect to complete before the
    /// commitment can be published onchain.
    WaitingForSetup {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// This validator's PoAP Merkle proof.
        poap: Vec<B256>,
        /// The block by which the commitment round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// A key generation is underway and the group's commitments are being
    /// collected onchain.
    CollectingCommitments {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// Key generation secrets, or `None` if not participating.
        secrets: Option<Box<Secrets>>,
        /// Verified commitments received from peers so far, keyed by
        /// participant.
        commitments: BTreeMap<Address, VerifiedCommitment>,
        /// The block by which the commitment round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// Every participant has committed and the group's secret shares are
    /// being collected onchain.
    CollectingShares {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// This validator's participation.
        participation: Box<KeyGenParticipation>,
        /// Verified participant public key shares.
        public_keys: BTreeMap<Address, PublicKeyShare>,
        /// Verified secret shares received from peers so far, keyed by
        /// participant.
        shares: BTreeMap<Address, VerifiedShare>,
        /// Complaints raised so far, keyed by accused participant.
        complaints: BTreeMap<Address, Complaint>,
        /// The block by which the secret-share round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// Every participant has submitted a secret share (valid or not) and the
    /// group's confirmations are being collected onchain.
    CollectingConfirmations {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// This validator's participation.
        participation: Box<KeyGenParticipation>,
        /// The status of the confirmation for the current validator.
        status: KeyGenConfirmation,
        /// Participants that have confirmed so far.
        confirmations: BTreeSet<Address>,
        /// Complaints raised so far, keyed by accused participant.
        complaints: BTreeMap<Address, Complaint>,
        /// The confirmation deadlines for the confirmation collection phase.
        /// `None` to indicate that there is no deadline.
        deadlines: Option<ConfirmationDeadlines>,
    },
    /// The next epoch's key generation completed and its rollover proposal is
    /// being signed by the active epoch.
    SigningRollover {
        /// The epoch being staged.
        next_epoch: NonZeroU64,
        /// The group being generated.
        group: Group,
        /// This validator's key share if they were participating.
        key_share: Option<Arc<KeyShare>>,
        /// The rollover proposal's signing hash.
        message: B256,
    },
}

/// The keygen participation status for the validator.
#[derive(Clone, Debug, Deserialize, Serialize)]
enum KeyGenParticipation {
    /// The validator is an active participant that is part of the keygen
    /// ceremony and holds its keygen sharing state.
    Participating(SharingState),
    /// The validator is an observer to the ceremony, and keeps track of the
    /// public commitments needed to verify publicly revealed shares in case of
    /// complaints as well as public participant public key shares.
    Observing(GroupCommitments),
}

/// The keygen secret share confirmation status.
#[derive(Clone, Debug, Deserialize, Serialize)]
enum KeyGenConfirmation {
    /// Keygen confirmation is being observed.
    Observing,
    /// Secret shares are still being collected, as there were some shares that
    /// could not be verified.
    Collecting(BTreeMap<Address, VerifiedShare>),
    /// All secret shares have been collected and a secret key share has been
    /// constructed.
    Confirmed(Arc<KeyShare>),
}

/// The complaints raised against a single accused participant.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct Complaint {
    /// The total number of complaints raised against the participant.
    total: u16,
    /// The number of complaints not yet responded to.
    unresponded: u16,
}

/// The deadlines for collecting confirmations.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ConfirmationDeadlines {
    /// The deadline to receive the last complaint.
    complain: u64,
    /// The deadline to receive the last complaint response.
    response: u64,
    /// The deadline to receive the last confirmation.
    confirm: u64,
}

/// A packet a validator group signs an attestation over.
#[derive(Clone, Debug, Deserialize, Serialize)]
enum Packet {
    /// A proposal to stage a newly generated epoch.
    EpochRollover {
        /// The epoch whose group signs the rollover.
        active_epoch: EpochId,
        /// The epoch being staged.
        proposed_epoch: NonZeroU64,
        /// The block at which the staged epoch becomes active.
        rollover_block: u64,
        /// The newly generated group's ID.
        group_id: B256,
        /// The newly generated group's public key.
        group_key: Point,
    },
    /// A proposed Safe transaction.
    Transaction {
        /// The epoch whose group signs the attestation.
        epoch: EpochId,
        /// The proposed transaction.
        transaction: SafeTransaction,
    },
    /// A proposed oracle-backed Safe transaction.
    OracleTransaction {
        /// The epoch whose group signs the attestation.
        epoch: EpochId,
        /// The oracle vouching for the transaction.
        oracle: Address,
        /// The proposed transaction.
        transaction: SafeTransaction,
    },
}

/// A signing session, keyed by the message hash the group signature attests
/// to.
// Every variant is a distinct wait state (for a request, an attestation, or
// to decline); the shared prefix reflects that, not redundant naming.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[allow(clippy::enum_variant_names)]
enum SigningState {
    /// The packet was verified; waiting for this validator's own `Sign`
    /// action to open the nonce-commitment round.
    WaitingForRequest {
        /// The key share for participating in the signing ceremony.
        key_share: Arc<KeyShare>,
        /// The packet being signed.
        packet: Packet,
        /// The group members expected to take part in signing.
        signers: BTreeSet<Address>,
        /// The block by which the signing round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// An oracle-backed packet's signing round is on hold until the oracle's
    /// result is attested.
    WaitingForOracle {
        /// The key share for participating in the signing ceremony.
        key_share: Arc<KeyShare>,
        /// The oracle whose result is awaited.
        oracle: Address,
        /// The group generating the signature.
        group_id: B256,
        /// The signature id assigned to this signing round.
        signature_id: B256,
        /// The nonce sequence number assigned to this signing round.
        sequence: u64,
        /// The packet being signed.
        packet: Packet,
        /// The group members expected to take part in signing.
        signers: BTreeSet<Address>,
        /// The block by which the oracle result must land. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// This validator has revealed its nonce commitment and is waiting for
    /// its peers to reveal theirs.
    CollectNonceCommitments {
        /// The key share for participating in the signing ceremony.
        key_share: Arc<KeyShare>,
        /// The group generating the signature.
        group_id: B256,
        /// The signature id assigned to this signing round.
        signature_id: B256,
        /// The nonce sequence number assigned to this signing round.
        sequence: u64,
        /// Verified revealed nonce commitments received from peers so far.
        revealed: BTreeMap<Address, RevealedNonces>,
        /// The packet being signed.
        packet: Packet,
        /// The group members expected to take part in signing.
        signers: BTreeSet<Address>,
        /// The block by which the commitment round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// Every signer's nonce commitment has been revealed and this
    /// validator's own signature share is being produced; waiting for the
    /// [`Effect::UseNonce`] effect to complete before it can be published.
    CollectSigningShares {
        /// The key share for participating in the signing ceremony.
        key_share: Arc<KeyShare>,
        /// The group generating the signature.
        group_id: B256,
        /// The signature id assigned to this signing round.
        signature_id: B256,
        /// Verified revealed nonce commitments received from peers so far.
        revealed: BTreeMap<Address, RevealedNonces>,
        /// The signing selections.
        selections: BTreeMap<MerkleRoot, SigningSelection>,
        /// The packet being signed.
        packet: Packet,
        /// The group members expected to take part in signing.
        signers: BTreeSet<Address>,
        /// The block by which the signature share round must complete.
        /// `None` to indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// A complete group signature was produced; waiting for the attestation
    /// to be submitted onchain.
    WaitingForAttestation {
        /// The signature id the completed signing round produced.
        signature_id: B256,
        /// The participant responsible for submitting the attestation, or
        /// `None` if unknown, in which case every participant is responsible.
        responsible: Option<Address>,
        /// The packet being signed.
        packet: Packet,
        /// The block by which the signing attestation must arrive.
        deadline: Option<u64>,
    },
    /// The packet failed verification; waiting to submit a decline.
    WaitingToDecline {
        /// The packet that failed verification.
        packet: Packet,
        /// The block by which the decline must be submitted. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
}

/// A Signing selection.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct SigningSelection {
    /// The participants that have published their signature share to this
    /// selection root.
    shares_from: BTreeSet<Address>,
    /// The last participant to publish a signature share, if any.
    last_signer: Option<Address>,
}

/// The pure validator state transition.
///
/// Holds the machine configuration the transition is parameterized over; the
/// group parameters it derives (identities, roots, thresholds) are all pure
/// functions of this configuration rather than snapshot state.
pub struct Transition {
    /// The account of the running validator.
    pub account: Address,
    /// The genesis participant set.
    pub genesis: ParticipantSet,
    /// The consensus signing domain.
    pub consensus: ConsensusDomain,
    /// The validator configuration.
    pub config: ValidatorConfig,
}

impl StateTransition<State> for Transition {
    type Event = Event;
    type Action = Action;
    type Effect = Effect;
    type Resume = Resume;

    fn apply_transition(
        &self,
        state: State,
        message: Message<Self::Event, Self::Resume>,
    ) -> (State, Commands<State, Self>) {
        match message {
            Message::Event(log) => match log.data {
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGen(event)) => {
                    self.handle_genesis_key_gen(state, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(event)) => {
                    self.handle_key_gen_committed(state, log.block, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenSecretShared(event)) => {
                    self.handle_key_gen_secret_shared(state, log.block, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenConfirmed(event)) => {
                    self.handle_key_gen_confirmed(state, log.block, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplained(event)) => {
                    self.handle_key_gen_complained(state, log.block, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplaintResponded(
                    event,
                )) => self.handle_key_gen_complaint_responded(state, log.block, &event),
                Event::Coordinator(Coordinator::CoordinatorEvents::Preprocess(event)) => {
                    self.handle_preprocess(state, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::Sign(event)) => {
                    self.handle_sign(state, log.block, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::SignRevealedNonces(event)) => {
                    self.handle_sign_revealed_nonces(state, log.block, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::SignShared(event)) => {
                    self.handle_sign_shared(state, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::SignCompleted(event)) => {
                    self.handle_sign_completed(state, log.block, &event)
                }
                Event::Consensus(Consensus::ConsensusEvents::EpochStaged(event)) => {
                    self.handle_epoch_staged(state, log.block, &event)
                }
                Event::Consensus(Consensus::ConsensusEvents::TransactionProposed(event)) => {
                    self.handle_transaction_proposed(state, log.block, &event)
                }
                Event::Consensus(Consensus::ConsensusEvents::TransactionAttested(event)) => {
                    self.handle_transaction_attested(state, &event)
                }
                Event::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(event)) => {
                    self.handle_oracle_transaction_proposed(state, log.block, &event)
                }
                Event::Consensus(Consensus::ConsensusEvents::OracleTransactionAttested(event)) => {
                    self.handle_oracle_transaction_attested(state, &event)
                }
                Event::Oracle(Oracle::OracleEvents::OracleResult(event)) => {
                    self.handle_oracle_result(state, log.block, log.address, &event)
                }
                // The remaining events are wired in as their handlers land.
                _ => (state, Vec::new()),
            },
            Message::NewBlock(block) => {
                let (state, rollover_commands) = self.handle_rollover_new_block(state, block);
                let (state, keygen_timeout_commands) = self.handle_key_gen_timeouts(state, block);
                (state, [rollover_commands, keygen_timeout_commands].concat())
            }
            Message::Resume(result) => match result {
                Resume::Noop => (state, Vec::new()),
                Resume::Setup { group_id, secrets } => {
                    self.handle_key_gen_setup(state, group_id, secrets)
                }
                Resume::NonceTree {
                    group_id,
                    commitment,
                } => self.handle_nonce_tree(state, group_id, commitment),
                Resume::NonceCommitments {
                    signature_id,
                    message,
                    nonces,
                    proof,
                } => self.handle_nonce_commitments(state, signature_id, message, nonces, proof),
                Resume::Nonce { message, nonces } => self.handle_nonces(state, message, nonces),
            },
        }
    }
}
