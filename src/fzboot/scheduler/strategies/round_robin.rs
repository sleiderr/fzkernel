use alloc::collections::vec_deque::VecDeque;

use crate::scheduler::task::TaskId;

use super::{SchedulingStrategy, TaskSchedulingMetadata};

pub struct RoundRobinScheduling {
    task_queue: VecDeque<RoundRobinMetadata>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RoundRobinMetadata {
    task_id: TaskId,
}

impl RoundRobinMetadata {
    pub fn new(task_id: TaskId) -> Self {
        Self { task_id }
    }
}

impl TaskSchedulingMetadata for RoundRobinMetadata {}

impl SchedulingStrategy<RoundRobinMetadata> for RoundRobinScheduling {
    fn next_task(&mut self) -> Option<TaskId> {
        let next_task = self.task_queue.pop_front().map(|meta| meta.task_id);

        if let Some(next_task) = next_task {
            self.insert_task(RoundRobinMetadata::new(next_task));
        }

        next_task
    }

    fn size(&self) -> usize {
        self.task_queue.len()
    }

    fn insert_task(&mut self, metadata: RoundRobinMetadata) {
        self.task_queue.push_back(metadata)
    }

    fn remove_task(&mut self, id: TaskId) {
        match self
            .task_queue
            .binary_search(&RoundRobinMetadata { task_id: id })
        {
            Ok(idx) => {
                self.task_queue.remove(idx);
            }
            Err(_) => (),
        }
    }

    fn init() -> Self {
        Self {
            task_queue: VecDeque::new(),
        }
    }
}
