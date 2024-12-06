use super::task::TaskId;

pub mod round_robin;

pub trait SchedulingStrategy<M: TaskSchedulingMetadata> {
    fn init() -> Self;
    fn next_task(&mut self) -> Option<TaskId>;
    fn size(&self) -> usize;
    fn insert_task(&mut self, _: M);
    fn remove_task(&mut self, id: TaskId);
}

pub trait TaskSchedulingMetadata {}
