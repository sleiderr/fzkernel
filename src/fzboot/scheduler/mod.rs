use core::sync::atomic::{AtomicUsize, Ordering};

use conquer_once::spin::OnceCell;
use fzproc_macros::interrupt_handler;
use queue::TaskQueue;
use spin::Mutex;
use strategies::round_robin::{RoundRobinMetadata, RoundRobinScheduling};
use task::{get_tasks, TaskId, TaskState, CURRENT_TASK_ID};

use crate::{
    error,
    irq::_pic_eoi,
    x86::{
        apic::InterruptVector,
        int::{disable_interrupts, enable_interrupts},
    },
};

use super::{
    irq::{manager::get_interrupt_manager, InterruptStackFrame},
    process::{
        get_process,
        thread::{get_thread, ThreadFlags, ThreadId},
        ProcessFlags, ProcessId,
    },
};

pub mod task;

pub mod queue;
pub mod strategies;

static GLOBAL_SCHEDULER: OnceCell<Mutex<GlobalScheduler>> = OnceCell::uninit();

pub static CURRENT_PROCESS_ID: AtomicUsize = AtomicUsize::new(0);
pub static CURRENT_THREAD_ID: AtomicUsize = AtomicUsize::new(0);

static FAILED_SCHEDULING: AtomicUsize = AtomicUsize::new(0);

#[interrupt_handler]
pub fn timer_irq_entry(frame: InterruptStackFrame) {
    if let Some(mut scheduler) = get_global_scheduler().try_lock() {
        let current_process = ProcessId::new(CURRENT_PROCESS_ID.load(Ordering::Relaxed));
        if let Some(process) = get_process(current_process) {
            // We cannot access information about the current process for some reason, we skip this scheduler run for this time.
            if let Some(process) = process.try_lock() {
                // we cannot pre-empt this process, skipping this run as well
                if process.flags.contains(ProcessFlags::NO_PREEMPT) {
                    return;
                }

                let current_thread = ThreadId::new(CURRENT_THREAD_ID.load(Ordering::Relaxed));
                if let Some(thread) = get_thread(current_thread) {
                    if let Some(thread) = thread.try_lock() {
                        if !thread.flags.contains(ThreadFlags::NO_PREEMPT) {
                            drop(thread);
                            drop(process);
                            FAILED_SCHEDULING.store(0, Ordering::Relaxed);
                            return scheduler.irq_schedule_next_task(frame);
                        }
                    }
                }
            }
            FAILED_SCHEDULING.fetch_add(1, Ordering::Relaxed);
            return;
        } else {
            FAILED_SCHEDULING.fetch_add(1, Ordering::Relaxed);
            error!("scheduler", "invalid current running process");
        }
    }

    // scheduler lock is held somewhere else, we cannot use it to update the current task
}

pub fn init_global_scheduler() {
    get_interrupt_manager().register_static_handler(InterruptVector::new(0x20), timer_irq_entry);
    get_global_scheduler()
        .lock()
        .schedule_sys_task(TaskId::new(0))
}

pub fn get_global_scheduler() -> &'static Mutex<GlobalScheduler> {
    GLOBAL_SCHEDULER.get_or_init(|| Mutex::new(GlobalScheduler::new()))
}

/// Returns the [`ThreadId`] of the currently executing [`Thread`]
pub fn current_thread_id() -> ThreadId {
    CURRENT_THREAD_ID.load(Ordering::Relaxed).into()
}

/// Returns the [`ProcessId`] of the currently executing [`Process`]
pub fn current_process_id() -> ProcessId {
    CURRENT_PROCESS_ID.load(Ordering::Relaxed).into()
}

pub struct GlobalScheduler {
    kernel_queue: TaskQueue<RoundRobinMetadata, RoundRobinScheduling>,
    count: usize,
}

impl GlobalScheduler {
    pub fn new() -> Self {
        Self {
            kernel_queue: TaskQueue::new(),
            count: 0,
        }
    }

    pub fn schedule_sys_task(&mut self, task_id: TaskId) {
        self.kernel_queue
            .queue_task(RoundRobinMetadata::new(task_id))
    }

    pub fn irq_schedule_next_task(&mut self, frame: InterruptStackFrame) {
        let next_task_id = self.kernel_queue.next_task();
        let current_task_id = TaskId::new(CURRENT_TASK_ID.load(Ordering::Relaxed));

        match next_task_id {
            Some(next_task_id) => {
                disable_interrupts();
                if current_task_id == next_task_id {
                    enable_interrupts();
                    return;
                }

                let tasks = get_tasks().read();
                let current_task_locked = tasks.get(&current_task_id);

                if let Some(current_task_locked) = current_task_locked {
                    let mut current_task = current_task_locked.lock();

                    current_task.state = TaskState::Waiting;
                    current_task.gpr = frame.registers;
                    current_task.rip = frame.rip;
                    current_task.stack = frame.stack_ptr;
                }

                let locked_next_task = match tasks.get(&next_task_id) {
                    Some(t) => t,
                    None => panic!("attempted to switch to a non-existent task"),
                };

                let mut next_task = locked_next_task.lock();

                if !matches!(next_task.state, TaskState::Uninitialized(_)) {
                    next_task.state = TaskState::Running;
                }

                let new_task_frame = InterruptStackFrame {
                    rip: next_task.rip.into(),
                    cs: frame.cs,
                    rflags: frame.rflags,
                    stack_segment: frame.stack_segment,
                    stack_ptr: next_task.stack.into(),
                    registers: next_task.gpr,
                };

                CURRENT_TASK_ID.store(next_task_id.into(), Ordering::Relaxed);
                CURRENT_PROCESS_ID.store(next_task.pid.into(), Ordering::Relaxed);
                CURRENT_THREAD_ID.store(next_task.tid.into(), Ordering::Relaxed);

                drop(next_task);
                drop(current_task_locked);
                drop(locked_next_task);
                drop(tasks);

                unsafe {
                    GLOBAL_SCHEDULER.get_unchecked().force_unlock();
                }

                _pic_eoi();

                unsafe {
                    new_task_frame.iret();
                }
            }
            None => {}
        }
    }
}
