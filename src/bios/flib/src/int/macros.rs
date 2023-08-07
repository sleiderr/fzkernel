
/// Set up the global Scheduler in a given variable
#[macro_export]
macro_rules! scheduler_ref {
    ($scheduler:ident) => {
        let mut $scheduler: &mut Box<$crate::int::scheduler::IntScheduler> = unsafe { transmute(SCHEDULER_ADDRESS) };
    }
}

