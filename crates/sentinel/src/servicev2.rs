use crate::{
    action::{SentinelActionKindV2 as SentinelActionKind, SentinelActionV2 as SentinelAction},
    bindings::{
        SentinelEventsV2 as SentinelEvents,
        consensus::Consensus,
        oracle::{ERC20, SentinelOracleV2 as SentinelOracle},
    },
    detector::Detector,
    state::StateV2 as State,
};
use alloy::{
    primitives::{Address, U256},
    sol_types::SolCall,
};
use safenet_core::{
    driver::{ActionEncoder, Service},
    state::{Command, Commands, Message, Pure, StateTransition},
    tx::{Signer, Transaction},
};
use std::convert::Infallible;

/// The sentinel service: drives the request FSM (mirroring
/// `SentinelOracleRequest.State`'s commit-reveal phases) from
/// `SentinelOracle`/`Consensus` events and maps its actions to encoded
/// transactions.
pub struct SentinelService {
    oracle: Address,
    fee_token: Address,
    consensus: Address,
    signer: Signer,
    chain_id: U256,
    voting_window: u64,
    detector: Detector,
}

/// Advances the request FSM in response to `SentinelOracle`/`Consensus`
/// events.
#[expect(dead_code)]
pub struct SentinelTransition {
    oracle: Address,
    /// The `Consensus` contract whose `OracleTransactionProposed` events are
    /// hashed into request ids.
    consensus: Address,
    /// Our own account, used to compute commitment hashes and identify votes
    /// we committed onchain.
    signer: Signer,
    /// The chain id of the EIP-712 domain used to derive request ids.
    chain_id: U256,
    /// The number of blocks a `WaitingForRequest` request is kept alive for
    /// before being cleaned up.
    voting_window: u64,
    detector: Detector,
}

/// Encodes [`SentinelAction`]s into the transactions that commit, reveal,
/// finalize and claim oracle requests.
pub struct SentinelEncoder {
    /// The `SentinelOracle` contract that commits, reveals, finalizations
    /// and claims are submitted to, and the spender approved to pull the
    /// bond.
    oracle: Address,
    /// The ERC-20 token that bonds are posted in.
    fee_token: Address,
}

#[expect(dead_code)]
impl SentinelService {
    pub fn new(
        oracle: Address,
        fee_token: Address,
        consensus: Address,
        signer: Signer,
        chain_id: U256,
        voting_window: u64,
        detector: Detector,
    ) -> Self {
        Self {
            oracle,
            fee_token,
            consensus,
            signer,
            chain_id,
            voting_window,
            detector,
        }
    }
}

impl SentinelTransition {
    // TODO(sentinel commit-reveal, service FSM sub-task B3b): starts
    // tracking a newly proposed oracle transaction, deciding whether we
    // vote to approve or deny it.
    fn handle_oracle_transaction_proposed(
        &self,
        _state: State,
        _block: u64,
        _event: Consensus::OracleTransactionProposed,
    ) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3b")
    }

    // TODO(sentinel commit-reveal, service FSM sub-task B3b): locks a bond
    // behind a blind commitment once a tracked request is opened for
    // commits onchain.
    fn handle_new_request(
        &self,
        _state: State,
        _event: SentinelOracle::NewRequest,
    ) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3b")
    }

    // TODO(sentinel commit-reveal, service FSM sub-task B3c): tallies a
    // commitment landing onchain, from any sentinel, for a request we're
    // still collecting commits for.
    fn handle_committed(
        &self,
        _state: State,
        _event: SentinelOracle::Committed,
    ) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3c")
    }

    // TODO(sentinel commit-reveal, service FSM sub-task B3c): tallies a
    // reveal landing onchain, from any sentinel, and early-finalizes once
    // every commit has been revealed.
    fn handle_revealed(
        &self,
        _state: State,
        _event: SentinelOracle::Revealed,
    ) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3c")
    }

    // TODO(sentinel commit-reveal, service FSM sub-task B3d): drops requests
    // we never got to commit on in time, reveals (or drops) requests past
    // their commit deadline, and finalizes requests past their reveal
    // deadline.
    fn handle_block_advance(&self, _state: State, _block: u64) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3d")
    }

    // TODO(sentinel commit-reveal, service FSM sub-task B3e): claims iff
    // our own revealed vote matches the oracle's outcome, dropping the
    // request either way.
    fn handle_resolved(
        &self,
        _state: State,
        _event: SentinelOracle::OracleResult,
    ) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3e")
    }
}

impl SentinelEncoder {
    fn encode_action_kind(&self, kind: SentinelActionKind) -> Transaction {
        match kind {
            SentinelActionKind::ApproveToken { bond } => Transaction {
                to: self.fee_token,
                value: U256::ZERO,
                data: ERC20::approveCall {
                    spender: self.oracle,
                    amount: bond,
                }
                .abi_encode()
                .into(),
                gas: 55_000,
            },
            // Measured onchain at ~196k gas for a request's first commit (fresh
            // storage slots for the request, the commitment and the ERC-20
            // allowance spend); 100k undershot this and ran out of gas. 250k
            // keeps headroom for `reveal`/`finalize`/`claim`'s own cold-storage
            // writes and the fee-token transfer.
            SentinelActionKind::Commit { id, hash } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitCall {
                    requestId: id,
                    commitHash: hash,
                }
                .abi_encode()
                .into(),
                gas: 250_000,
            },
            SentinelActionKind::Reveal { id, approve, salt } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::revealCall {
                    requestId: id,
                    approve,
                    salt,
                }
                .abi_encode()
                .into(),
                gas: 250_000,
            },
            SentinelActionKind::Finalize { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::finalizeCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 250_000,
            },
            SentinelActionKind::Claim { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::claimCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 250_000,
            },
        }
    }
}

impl StateTransition<State> for SentinelTransition {
    type Event = SentinelEvents;
    type Action = SentinelAction;
    type Effect = Infallible;
    type Resume = Infallible;

    fn apply_transition(
        &self,
        state: State,
        message: Message<Self::Event, Self::Resume>,
    ) -> (State, Commands<State, Self>) {
        let (state, actions) = match message {
            Message::NewBlock(block) => self.handle_block_advance(state, block),
            Message::Event(event) => {
                let block = event.block;
                match event.data {
                    SentinelEvents::Consensus(
                        Consensus::ConsensusEvents::OracleTransactionProposed(event),
                    ) => self.handle_oracle_transaction_proposed(state, block, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::NewRequest(
                        event,
                    )) => self.handle_new_request(state, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::Committed(
                        event,
                    )) => self.handle_committed(state, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::Revealed(
                        event,
                    )) => self.handle_revealed(state, event),
                    SentinelEvents::Oracle(
                        SentinelOracle::SentinelOracleV2Events::OracleResult(event),
                    ) => self.handle_resolved(state, event),
                }
            }
            Message::Resume(result) => match result {},
        };
        let commands = actions.into_iter().map(Command::Action).collect();
        (state, commands)
    }
}

impl ActionEncoder<SentinelAction> for SentinelEncoder {
    fn encode_action(&self, action: SentinelAction) -> (Transaction, Option<u64>) {
        (self.encode_action_kind(action.kind), action.expires_at)
    }
}

impl Service for SentinelService {
    type State = State;
    type Event = SentinelEvents;

    type Transition = SentinelTransition;
    type Effects = Pure;
    type Actions = SentinelEncoder;

    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions) {
        let SentinelService {
            oracle,
            fee_token,
            consensus,
            signer,
            chain_id,
            voting_window,
            detector,
        } = self;
        (
            SentinelTransition {
                oracle,
                consensus,
                signer,
                chain_id,
                voting_window,
                detector,
            },
            Pure,
            SentinelEncoder { oracle, fee_token },
        )
    }
}
