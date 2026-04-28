use anyhow::Result;

use crate::{actions, config::ValidatorConfig, state::ValidatorState, watcher};

#[derive(Default)]
pub struct Driver {
    state: ValidatorState,
    actions: actions::Handler,
}

impl Driver {
    pub async fn run(&mut self, config: &ValidatorConfig) -> Result<()> {
        watcher::run(config, |update| self.on_update(update)).await
    }

    fn on_update(&mut self, update: watcher::Update) {
        let mut actions = self.state.on_block(update.block_number);
        for event in update.events {
            let new_actions = match event {
                watcher::Event::Consensus(e) => self.state.on_consensus_event(e),
                watcher::Event::Coordinator(e) => self.state.on_coordinator_event(e),
            };
            actions.extend(new_actions);
        }
        self.actions.handle(actions);
    }
}
