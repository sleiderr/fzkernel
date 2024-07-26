use core::sync::atomic::Ordering;

use conquer_once::spin::OnceCell;
use fzproc_macros::interrupt_handler;
use queue::TaskQueue;
use spin::Mutex;
use strategies::round_robin::{RoundRobinMetadata, RoundRobinScheduling};
use task::{get_tasks, TaskId, TaskState, CURRENT_TASK_ID};

use crate::{
    irq::_pic_eoi,
    println,
    video::vesa::text_buffer,
    x86::{
        apic::InterruptVector,
        int::{disable_interrupts, enable_interrupts},
    },
};

use super::irq::{manager::get_interrupt_manager, InterruptStackFrame};

pub mod task;

pub mod queue;
pub mod strategies;

static GLOBAL_SCHEDULER: OnceCell<Mutex<GlobalScheduler>> = OnceCell::uninit();

#[interrupt_handler]
pub fn timer_irq_entry(frame: InterruptStackFrame) {
    unsafe {
        get_global_scheduler().force_unlock();
    }
    get_global_scheduler().lock().irq_schedule_next_task(frame);
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

                next_task.state = TaskState::Running;

                let new_task_frame = InterruptStackFrame {
                    rip: next_task.rip.into(),
                    cs: frame.cs,
                    rflags: frame.rflags,
                    stack_segment: frame.stack_segment,
                    stack_ptr: next_task.stack.into(),
                    registers: next_task.gpr,
                };

                CURRENT_TASK_ID.store(next_task_id.into(), Ordering::Relaxed);

                drop(next_task);
                drop(current_task_locked);
                drop(locked_next_task);
                drop(tasks);

                _pic_eoi();

                unsafe {
                    new_task_frame.iret();
                }
            }
            None => {}
        }
    }
}
