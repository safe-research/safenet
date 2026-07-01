use crate::{
    action::{SentinelAction, SentinelActionKind},
    bindings::{
        SentinelEvents,
        consensus::Consensus,
        oracle::{ERC20, SentinelOracle},
    },
    state::State,
};
use alloy::{
    primitives::{Address, U256},
    sol_types::SolCall,
};
use safenet_core::{Service, index::EventLog, state::StateTransition, tx::Transaction};

/// The sentinel service: drives the request FSM (`preparing -> pending ->
/// committed -> finalized`) from `SentinelOracle`/`Consensus` events and maps
/// its actions to encoded transactions.
pub struct SentinelService {
    oracle: Address,
    fee_token: Address,
}

impl SentinelService {
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn new(oracle: Address, fee_token: Address) -> Self {
        Self { oracle, fee_token }
    }

    /// Starts tracking a newly proposed oracle transaction, deciding whether
    /// we vote to approve or deny it.
    fn handle_oracle_transaction_proposed(
        &self,
        state: State,
        _event: Consensus::OracleTransactionProposed,
    ) -> (State, Vec<SentinelAction>) {
        // TODO: gate on our oracle, derive the request id, run the detector
        // and start tracking the request as `Preparing`.
        (state, Vec::new())
    }

    /// Casts our vote once a tracked request is opened for voting onchain.
    fn handle_new_request(
        &self,
        state: State,
        _event: SentinelOracle::NewRequest,
    ) -> (State, Vec<SentinelAction>) {
        // TODO: only act on a `Preparing` request; emit `ApproveToken` and
        // the vote, and move it to `Pending`.
        (state, Vec::new())
    }

    /// Records that our vote has been committed onchain.
    fn handle_committed(
        &self,
        state: State,
        _event: SentinelOracle::Committed,
    ) -> (State, Vec<SentinelAction>) {
        // TODO: gate on our own account; only move a `Pending` request to
        // `Committed`.
        (state, Vec::new())
    }

    /// Claims the bond for a request we committed on, once its outcome is
    /// known.
    fn handle_resolved(
        &self,
        state: State,
        _event: SentinelOracle::OracleResult,
    ) -> (State, Vec<SentinelAction>) {
        // TODO: drop the request unconditionally; only emit a `Claim` when
        // we actually committed onchain (`Committed`/`Finalized`) and either
        // the vote won or the request timed out.
        (state, Vec::new())
    }

    /// Finalizes committed requests and cleans up stale ones once their
    /// voting deadline has passed.
    fn handle_block_advance(&self, state: State, _block: u64) -> (State, Vec<SentinelAction>) {
        // TODO: move past-deadline `Committed` requests to `Finalized` with
        // a `Finalize` action; drop stale `Preparing`/`Pending`/`Finalized`
        // requests.
        (state, Vec::new())
    }

    fn encode_action(&self, kind: SentinelActionKind) -> Transaction {
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
            SentinelActionKind::CommitApprove { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitApproveCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::CommitDeny { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitDenyCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::Finalize { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::finalizeCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::Claim { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::claimCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
        }
    }
}

impl StateTransition<State> for SentinelService {
    type Event = SentinelEvents;
    type Action = SentinelAction;

    async fn new_block(&mut self, state: State, block: u64) -> (State, Vec<Self::Action>) {
        self.handle_block_advance(state, block)
    }

    async fn event(
        &mut self,
        state: State,
        event: EventLog<Self::Event>,
    ) -> (State, Vec<Self::Action>) {
        match event.data {
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                event,
            )) => self.handle_oracle_transaction_proposed(state, event),
            SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(event)) => {
                self.handle_new_request(state, event)
            }
            SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Committed(event)) => {
                self.handle_committed(state, event)
            }
            SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::OracleResult(event)) => {
                self.handle_resolved(state, event)
            }
        }
    }
}

impl Service for SentinelService {
    type State = State;

    /// Maps each action to a `(Transaction, expires_at)` pair for the queue.
    ///
    /// The `expires_at` is the request's voting deadline, forwarded from the
    /// action so the `TransactionQueue` can drop it if it goes unsubmitted
    /// past that block.
    fn encode_actions(&self, actions: Vec<SentinelAction>) -> Vec<(Transaction, u64)> {
        actions
            .into_iter()
            .map(|SentinelAction { kind, expires_at }| (self.encode_action(kind), expires_at))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{B256, address, uint};

    const ORACLE: Address = address!("1111111111111111111111111111111111111111");
    const FEE_TOKEN: Address = address!("2222222222222222222222222222222222222222");

    fn service() -> SentinelService {
        SentinelService::new(ORACLE, FEE_TOKEN)
    }

    #[test]
    fn encodes_approve_token() {
        let bond = uint!(1_000_U256);
        let tx = service().encode_action(SentinelActionKind::ApproveToken { bond });

        assert_eq!(tx.to, FEE_TOKEN);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 55_000);
        assert_eq!(
            tx.data.as_ref(),
            ERC20::approveCall {
                spender: ORACLE,
                amount: bond
            }
            .abi_encode(),
        );
    }

    #[test]
    fn encodes_commit_approve() {
        let id = B256::repeat_byte(0x01);
        let tx = service().encode_action(SentinelActionKind::CommitApprove { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::commitApproveCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_commit_deny() {
        let id = B256::repeat_byte(0x02);
        let tx = service().encode_action(SentinelActionKind::CommitDeny { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::commitDenyCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_finalize() {
        let id = B256::repeat_byte(0x03);
        let tx = service().encode_action(SentinelActionKind::Finalize { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::finalizeCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_claim() {
        let id = B256::repeat_byte(0x04);
        let tx = service().encode_action(SentinelActionKind::Claim { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::claimCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encode_actions_forwards_expiry() {
        let bond = uint!(500_U256);
        let id = B256::repeat_byte(0xab);
        let deadline = 999u64;
        let encoded = service().encode_actions(vec![
            SentinelAction {
                kind: SentinelActionKind::ApproveToken { bond },
                expires_at: deadline,
            },
            SentinelAction {
                kind: SentinelActionKind::CommitApprove { id },
                expires_at: deadline,
            },
        ]);

        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].0.to, FEE_TOKEN);
        assert_eq!(encoded[1].0.to, ORACLE);
        assert_eq!(encoded[0].1, deadline);
        assert_eq!(encoded[1].1, deadline);
    }
}
