use crate::{
    bindings::{Coordinator, KeyGenCommitment, KeyGenSecretShare},
    config::{addresses::Addresses, chain::Chain, provider::Provider},
};
use alloy::{
    consensus::{SignableTransaction as _, TxEip1559},
    network::TxSignerSync as _,
    primitives::{Address, B256, TxKind},
    providers::Provider as _,
    signers::local::PrivateKeySigner,
    sol_types::SolCall as _,
    eips::Encodable2718 as _,
};
use anyhow::Result;
use tokio::{sync::mpsc, task::JoinHandle};

#[allow(clippy::enum_variant_names)]
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
    KeyGenSecretShare {
        gid: B256,
        share: KeyGenSecretShare,
    },
    KeyGenConfirm {
        gid: B256,
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
            Self::KeyGenSecretShare { gid, share } => EncodedAction {
                to: addresses.coordinator,
                data: Coordinator::keyGenSecretShareCall { gid, share }.abi_encode(),
                gas_limit: 500_000,
            },
            Self::KeyGenConfirm { gid } => EncodedAction {
                to: addresses.coordinator,
                data: Coordinator::keyGenConfirmCall { gid }.abi_encode(),
                gas_limit: 300_000,
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
        let mut tx = TxEip1559 {
            chain_id: self.chain.id(),
            nonce,
            gas_limit: encoded.gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            to: TxKind::Call(encoded.to),
            input: encoded.data.into(),
            ..Default::default()
        };
        let signature = self.signer.sign_transaction_sync(&mut tx)?;
        let raw_tx = tx.into_signed(signature).encoded_2718();
        let tx_hash = self.provider.send_raw_transaction(&raw_tx).await?.watch().await?;
        tracing::debug!(%tx_hash, "executed action transaction");
        Ok(())
    }
}
