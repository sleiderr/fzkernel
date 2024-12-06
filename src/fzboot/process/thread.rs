use core::{
    ops::{BitAnd, BitOr, BitOrAssign},
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::sync::Arc;
use conquer_once::spin::OnceCell;
use hashbrown::{HashMap, HashSet};
use spin::{Mutex, RwLock};

use crate::{
    mem::VirtAddr,
    scheduler::{
        current_thread_id, get_global_scheduler,
        task::{get_task, Task, TaskId, TaskState::Uninitialized},
    },
};

use super::{get_process, ProcessId, __process_init};

static FIRST_AVAILABLE_TGID: AtomicUsize = AtomicUsize::new(1);
static FIRST_AVAILABLE_TID: AtomicUsize = AtomicUsize::new(1);

pub(super) static THREAD_REGISTRY: OnceCell<RwLock<HashMap<ThreadId, Arc<Mutex<Thread>>>>> =
    OnceCell::uninit();

pub fn get_thread(thread_id: ThreadId) -> Option<Arc<Mutex<Thread>>> {
    unsafe {
        THREAD_REGISTRY
            .get_unchecked()
            .read()
            .get(&thread_id)
            .cloned()
    }
}

pub fn __thread_init() -> ! {
    let thread_info_locked =
        get_thread(current_thread_id()).expect("failed to load current thread context");

    let thread_info = thread_info_locked.lock();

    let task_state = thread_info.task.lock().state;

    drop(thread_info);

    match task_state {
        Uninitialized(entry) => {
            let thread_entry_fn =
                unsafe { core::mem::transmute::<*const u8, fn() -> !>(entry.as_ptr::<u8>()) };

            thread_entry_fn()
        }
        _ => {
            panic!("attempt to initialize already initialized thread")
        }
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThreadId(usize);

impl ThreadId {
    pub const KERNEL_INIT_TID: Self = Self(0);

    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

impl From<ThreadId> for usize {
    fn from(value: ThreadId) -> Self {
        value.0
    }
}

impl From<usize> for ThreadId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub struct Thread {
    pub(crate) id: ThreadId,
    pub(super) task: Arc<Mutex<Task>>,
    pub flags: ThreadFlags,
}

impl Thread {
    /// Returns the [`ThreadId`] associated to this `Thread`.
    ///
    /// It uniquely identified this specific thread.
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// Schedules this `Thread` for execution.
    ///
    /// When first created, threads are usually not directly scheduled and therefore won't run until registered in the
    /// global system scheduler.
    pub fn schedule(&self) {
        get_global_scheduler()
            .lock()
            .schedule_sys_task(self.task.lock().id);
    }

    fn spawn_thread(
        pid: ProcessId,
        thread_entry: VirtAddr,
        thread_flags: ThreadFlags,
        task_init_fn: fn(fn() -> !, fn() -> !, ThreadId) -> TaskId,
        init_fn: fn() -> !,
    ) -> Arc<Mutex<Self>> {
        let tid = ThreadId(FIRST_AVAILABLE_TID.fetch_add(1, Ordering::Relaxed));

        let thread_entry_fn =
            unsafe { core::mem::transmute::<*const u8, fn() -> !>(thread_entry.as_ptr::<u8>()) };

        let mut thread_task = get_task(task_init_fn(init_fn, thread_entry_fn, tid)).unwrap();

        let mut thread = Thread {
            id: tid,
            task: thread_task,
            flags: thread_flags,
        };

        let process = get_process(pid).unwrap().lock().threads.insert_thread(tid);

        let thread = Arc::new(Mutex::new(thread));

        unsafe {
            THREAD_REGISTRY
                .get_unchecked()
                .write()
                .insert(tid, thread.clone());
        }

        thread
    }

    /// Creates the main (first) [`Process`] `Thread`.
    ///
    /// This thread is in charge of performing process' initialization when first scheduled, and then calls the process entry
    /// point.
    pub fn spawn_process_main_thread(
        pid: ProcessId,
        thread_entry: VirtAddr,
        thread_flags: ThreadFlags,
    ) -> Arc<Mutex<Self>> {
        Self::spawn_thread(
            pid,
            thread_entry,
            thread_flags,
            Task::init_kernel_task,
            __process_init,
        )
    }

    pub fn spawn_kernel_thread(thread_entry: VirtAddr) -> Arc<Mutex<Self>> {
        Self::spawn_thread(
            ProcessId::KERNEL_INIT_PID,
            thread_entry,
            ThreadFlags::default(),
            Task::init_kernel_task,
            __thread_init,
        )
    }
}

/// A `ThreadGroup` represents a list of threads that are linked together for some reason.
///
/// Usually, a `ThreadGroup` is associated with a process and lists all existing threads for that process.
#[derive(Debug)]
pub struct ThreadGroup {
    id: usize,
    threads: HashSet<ThreadId>,
}

impl ThreadGroup {
    /// Creates a new empty `ThreadGroup`, with a unique identifier across the system.
    pub fn new_empty() -> Self {
        let tg_id = FIRST_AVAILABLE_TGID.fetch_add(1, Ordering::Relaxed);

        Self {
            id: tg_id,
            threads: HashSet::new(),
        }
    }

    /// Registers a new [`Thread`] to this `ThreadGroup`.
    ///
    /// Does nothing if the [`Thread`] identified by `thread_id` already belongs to the group.
    pub fn insert_thread(&mut self, thread_id: ThreadId) {
        self.threads.insert(thread_id);
    }

    /// Deletes a [`Thread`] from this `ThreadGroup`.
    ///
    /// Does nothing if the [`Thread`] identified by `thread_id` does not belong to the group.
    pub fn remove_thread(&mut self, thread_id: ThreadId) {
        self.threads.remove(&thread_id);
    }
}

/// Unique identifier associated with a [`ThreadGroup`]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadGroupId(usize);

impl ThreadGroupId {
    /// First [`ThreadGroup`] id, assigned to the Kernel process' thread group.
    pub const KERNEL_INIT_TGID: Self = Self(0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadFlags(u64);

impl ThreadFlags {
    const NO_FLAGS: Self = Self(0);

    /// If this flag is set, the [`Process`] cannot be pre-empted, and will run until it manually yields back control to
    /// the scheduler.
    pub const NO_PREEMPT: Self = Self(1 << 32);

    pub fn contains(self, mode: Self) -> bool {
        self & mode != Self::NO_FLAGS
    }
}

impl BitAnd for ThreadFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for ThreadFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ThreadFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0
    }
}
