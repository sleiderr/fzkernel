use core::marker::PhantomData;

use super::{
    strategies::{SchedulingStrategy, TaskSchedulingMetadata},
    task::TaskId,
};

pub struct TaskQueue<M: TaskSchedulingMetadata, Q: SchedulingStrategy<M>> {
    strategy: Q,
    _metadata: PhantomData<M>,
}

impl<M: TaskSchedulingMetadata, Q: SchedulingStrategy<M>> TaskQueue<M, Q> {
    pub fn new() -> Self {
        Self {
            strategy: Q::init(),
            _metadata: PhantomData,
        }
    }

    pub fn next_task(&mut self) -> Option<TaskId> {
        self.strategy.next_task()
    }

    pub fn queue_task(&mut self, task_metadata: M) {
        self.strategy.insert_task(task_metadata)
    }
}
