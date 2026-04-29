use alloy::primitives::B256;

pub enum Action {
    KeyGenAndCommit { gid: B256 },
}

#[derive(Default)]
pub struct Handler;

impl Handler {
    pub fn handle(&mut self, _actions: Vec<Action>) {}
}
