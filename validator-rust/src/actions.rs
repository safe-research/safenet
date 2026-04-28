pub enum Action {}

#[derive(Default)]
pub struct Handler;

impl Handler {
    pub fn handle(&mut self, _actions: Vec<Action>) {}
}
