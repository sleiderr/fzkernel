#[derive(Default)]
pub struct AppState {
    step_id: usize,
    action_id: usize,
}

impl AppState {
    pub fn set_step_id(&mut self, step_id: usize) {
        self.step_id = step_id;
    }

    pub fn set_action_id(&mut self, action_id: usize) {
        self.action_id = action_id;
    }

    pub fn action_id(&self) -> usize {
        self.action_id
    }
}
