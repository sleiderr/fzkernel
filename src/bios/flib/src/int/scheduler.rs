use alloc::collections::VecDeque;

pub struct ScheduledAction {
    action : usize,
}

impl ScheduledAction {
    /// Creates a new [`ScheduledAction`] given a value
    pub fn new(value : usize) -> Self {
        Self {
            action: value,
        }
    }
}

pub struct IntScheduler {
    queue : VecDeque<ScheduledAction>
}

impl IntScheduler {
    /// Schedules a given action by pushing it in the queue
    pub fn schedule(&mut self, action : ScheduledAction) {
        self.queue.push_back(action);
    }
}