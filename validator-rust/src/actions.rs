use alloy::primitives::B256;

use crate::bindings::KeyGenCommitment;

pub enum Action {
    KeyGenAndCommit {
        gid: B256,
        participants_root: B256,
        count: u16,
        threshold: u16,
        context: B256,
        poap: Vec<B256>,
        commitment: KeyGenCommitment,
    },
}

#[derive(Default)]
pub struct Handler;

impl Handler {
    pub fn handle(&mut self, _actions: Vec<Action>) {}
}
