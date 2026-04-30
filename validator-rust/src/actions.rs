use crate::{
    bindings::{Coordinator, KeyGenCommitment},
    config::{addresses::Addresses, chain::Chain, provider::Provider},
};
use alloy::{
    consensus::TxEip1559,
    network::TxSignerSync as _,
    primitives::{Address, B256, TxKind},
    providers::Provider as _,
    signers::local::PrivateKeySigner,
    sol_types::SolCall as _,
};
use anyhow::Result;
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Clone)]
pub enum Action {
    KeyGenAndCommit {
        participants: B256,
        count: u16,
        threshold: u16,
        context: B256,
        poap: Vec<B256>,
        commitment: KeyGenCommitment,
    },
}

struct EncodedAction {
    to: Address,
    data: Vec<u8>,
    gas_limit: u64,
}

impl Action {
    fn into_encoded(self, addresses: &Addresses) -> EncodedAction {
        match self {
            Self::KeyGenAndCommit {
                participants,
                count,
                threshold,
                context,
                poap,
                commitment,
            } => EncodedAction {
                to: addresses.coordinator,
                data: Coordinator::keyGenAndCommitCall {
                    participants,
                    count,
                    threshold,
                    context,
                    poap,
                    commitment,
                }
                .abi_encode(),
                gas_limit: 250_000,
            },
        }
    }
}

pub struct Handler {
    sender: mpsc::UnboundedSender<Vec<Action>>,
    _handle: JoinHandle<()>,
}

impl Handler {
    pub fn new(
        provider: Provider,
        signer: PrivateKeySigner,
        chain: Chain,
        addresses: Addresses,
    ) -> Self {
        let worker = Worker {
            provider,
            signer,
            chain,
            addresses,
        };
        let (sender, receiver) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move { worker.run(receiver).await });

        Self {
            sender,
            _handle: handle,
        }
    }

    pub fn handle(&mut self, actions: Vec<Action>) {
        self.sender
            .send(actions)
            .expect("channel unexpectedly closed");
    }
}

struct Worker {
    provider: Provider,
    signer: PrivateKeySigner,
    chain: Chain,
    addresses: Addresses,
}

impl Worker {
    async fn run(self, mut receiver: mpsc::UnboundedReceiver<Vec<Action>>) {
        while let Some(actions) = receiver.recv().await {
            for action in actions {
                if let Err(err) = self.handle_action(action).await {
                    tracing::warn!(%err, "dropping action");
                }
            }
        }
    }

    async fn handle_action(&self, action: Action) -> Result<()> {
        let encoded = action.into_encoded(&self.addresses);
        let (nonce, max_fee_per_gas, max_priority_fee_per_gas) = tokio::try_join!(
            self.provider.get_transaction_count(self.signer.address()),
            self.provider.get_gas_price(),
            self.provider.get_max_priority_fee_per_gas(),
        )?;
        self.signer.sign_transaction_sync(&mut TxEip1559 {
            chain_id: self.chain.id(),
            nonce,
            gas_limit: encoded.gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            to: TxKind::Call(encoded.to),
            input: encoded.data.into(),
            ..Default::default()
        })?;
        Ok(())
    }
}
