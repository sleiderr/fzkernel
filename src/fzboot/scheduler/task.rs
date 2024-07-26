use core::{
    arch::asm,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::{collections::btree_map::BTreeMap, sync::Arc};
use conquer_once::spin::OnceCell;
use spin::{rwlock::RwLock, Mutex};

use crate::{
    mem::{stack::get_kernel_stack_allocator, VirtAddr},
    process::{get_process, Process, ProcessId},
    x86::registers::x86_64::GeneralPurposeRegisters,
};

type LockedTaskTree = RwLock<BTreeMap<TaskId, Arc<Mutex<Task>>>>;

static TASKS: OnceCell<LockedTaskTree> = OnceCell::uninit();

/// Returns the system`s [`Task`] directory.
///
/// Contains every existing task, running or not on the system.
pub fn get_tasks() -> &'static LockedTaskTree {
    TASKS.get_or_init(|| {
        let mut task_map = BTreeMap::new();

        task_map.insert(TaskId::INIT_TASK, Arc::new(Mutex::new(Task::default())));

        RwLock::new(task_map)
    })
}

pub fn get_task(task_id: TaskId) -> Option<Arc<Mutex<Task>>> {
    get_tasks().read().get(&task_id).cloned()
}

pub fn get_current_task() -> Arc<Mutex<Task>> {
    get_task(TaskId::new(CURRENT_TASK_ID.load(Ordering::Relaxed))).unwrap()
}

/// First available [`Task`] ID.
static LAST_TASK_ID: AtomicUsize = AtomicUsize::new(1);

/// [`Task`] ID of the current running task.
pub static CURRENT_TASK_ID: AtomicUsize = AtomicUsize::new(0);

/// A unique identifier is associated with every task.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(usize);

impl TaskId {
    /// Task ID of the first Kernel task created.
    pub const INIT_TASK: Self = Self(0);

    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

impl From<TaskId> for u64 {
    fn from(value: TaskId) -> Self {
        u64::try_from(value.0).expect("invalid task id")
    }
}

impl From<TaskId> for usize {
    fn from(value: TaskId) -> Self {
        value.0
    }
}

/// A `Task` represents a code execution context.
///
/// This structure contains all information needed to perform a task switch (content of the CPU registers, stack-related information,
/// virtual memory mappings).
#[derive(Debug, Default)]
pub struct Task {
    pub(crate) id: TaskId,
    pub(crate) pid: ProcessId,
    pub(super) state: TaskState,
    pub(super) kernel_stack: VirtAddr,
    pub(super) stack: VirtAddr,
    pub(super) rip: VirtAddr,
    pub(super) gpr: GeneralPurposeRegisters,
}

impl Task {
    /// Creates a new Kernel `Task` and schedules it for execution.
    pub fn init_kernel_task(entry_fn: fn() -> !) -> TaskId {
        let task_id = TaskId(LAST_TASK_ID.fetch_add(1, Ordering::Relaxed));
        let entry_fn_addr = VirtAddr::new(entry_fn as u64);

        let mut task = Task {
            id: task_id,
            pid: ProcessId::KERNEL_INIT_PID,
            state: TaskState::Uninitialized,
            rip: entry_fn_addr,
            ..Default::default()
        };

        let kernel_stack = get_kernel_stack_allocator().lock().alloc_stack();

        task.stack = kernel_stack;

        get_tasks()
            .write()
            .insert(task_id, Arc::new(Mutex::new(task)));

        task_id
    }

    pub fn process(&self) -> Arc<Mutex<Process>> {
        get_process(self.pid).expect("attempted to access process of orphan task")
    }
}

#[derive(Debug)]
struct TaskStateSnapshot {
    gpr: GeneralPurposeRegisters,
    rsp: u64,
    rip: u64,
    task_id: u64,
}

/// Current running status of a [`Task`]
#[derive(Debug, Default)]
pub enum TaskState {
    /// The [`Task`] is currently being executed by a CPU.
    Running,

    /// The [`Task`] is waiting to be scheduled for execution.
    Waiting,

    /// This [`Task`] is new and never got any CPU time allocated.
    #[default]
    Uninitialized,
}

/// Performs a task switch, manually changing the current execution context to another task.
///
/// This only requires the [`TaskId`] of the [`Task`] to be scheduled.
#[macro_export]
macro_rules! task_switch {
    ($task_id: tt) => {
        unsafe {
            core::arch::asm!(
                "push rax",
                "call __task_state_snapshot",
                in("rax") u64::from($task_id)
            )
        }
    };
}

/// Saves the execution context of the currently running [`Task`], before attempting the task switch.
#[no_mangle]
#[naked]
pub unsafe extern "C" fn __task_state_snapshot() {
    asm!(
        "push rsp",
        "push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rbp
        push rdi
        push rsi
        push rdx
        push rcx
        push rbx
        push rax
        ",
        "mov rdi, rsp",
        "call perform_task_switch",
        "mov rdx, rdx",
        "ret",
        options(noreturn)
    )
}

#[no_mangle]
fn perform_task_switch(state: TaskStateSnapshot) {
    let tasks = get_tasks().read();
    let next_task_id = TaskId(usize::try_from(state.task_id).expect("invalid task id"));
    let current_task_id =
        TaskId(usize::try_from(CURRENT_TASK_ID.load(Ordering::Relaxed)).expect("invalid task id"));

    let locked_next_task = match tasks.get(&next_task_id) {
        Some(t) => t,
        None => panic!("attempted to switch to a non-existent task"),
    };

    let locked_current_task = match tasks.get(&current_task_id) {
        Some(t) => t,
        None => {
            panic!("could not find current stack context")
        }
    };

    let mut current_task = locked_current_task.lock();

    let mut next_task = locked_next_task.lock();

    current_task.gpr = state.gpr;
    current_task.stack = VirtAddr::from(state.rsp);
    current_task.rip = VirtAddr::from(state.rip);
    current_task.state = TaskState::Waiting;

    next_task.state = TaskState::Running;

    let new_task_state = TaskStateSnapshot {
        gpr: next_task.gpr,
        rsp: next_task.stack.into(),
        rip: next_task.rip.into(),
        task_id: next_task.id.into(),
    };

    CURRENT_TASK_ID.store(next_task_id.0, Ordering::Relaxed);

    drop(next_task);
    drop(current_task);
    drop(tasks);

    __restore_task_state(new_task_state);
}

/// Loads the execution context of the next [`Task`] scheduled for execution.
#[no_mangle]
fn __restore_task_state(state: TaskStateSnapshot) -> ! {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax, [rdi + 0x0]",
            "mov rbx, [rdi + 0x8]",
            "mov rcx, [rdi + 0x10]",
            "mov rdx, [rdi + 0x18]",
            "mov rsi, [rdi + 0x20]",
            "mov rbp, [rdi + 0x30]",
            "mov r8, [rdi + 0x38]",
            "mov r9, [rdi + 0x40]",
            "mov r10, [rdi + 0x48]",
            "mov r11, [rdi + 0x50]",
            "mov r12, [rdi + 0x58]",
            "mov r13, [rdi + 0x60]",
            "mov r14, [rdi + 0x68]",
            "mov r15, [rdi + 0x70]",
            "mov rsp, [rdi + 0x78]",
            "add rsp, 8",
            "push [rdi + 0x80]",
            "ret",
            in("rdi") core::ptr::addr_of!(state),
            options(noreturn)
        )
    }

    unreachable!();
}
