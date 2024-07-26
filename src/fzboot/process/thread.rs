use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::sync::Arc;
use conquer_once::spin::OnceCell;
use hashbrown::{HashMap, HashSet};
use spin::{Mutex, RwLock};

use crate::{
    mem::VirtAddr,
    scheduler::{
        get_global_scheduler,
        task::{get_task, Task},
    },
};

use super::{get_process, ProcessId};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThreadId(usize);

impl ThreadId {
    pub const KERNEL_INIT_TID: Self = Self(0);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadGroupId(usize);

impl ThreadGroupId {
    pub const KERNEL_INIT_TGID: Self = Self(0);
}

pub struct Thread {
    pub(super) id: ThreadId,
    pub(super) task: Arc<Mutex<Task>>,
}

impl Thread {
    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub fn schedule(&self) {
        get_global_scheduler()
            .lock()
            .schedule_sys_task(self.task.lock().id);
    }

    pub fn spawn_kernel_thread(thread_entry: VirtAddr) -> Arc<Mutex<Self>> {
        let tid = ThreadId(FIRST_AVAILABLE_TID.fetch_add(1, Ordering::Relaxed));

        let mut thread_task = get_task(Task::init_kernel_task(unsafe {
            core::mem::transmute::<*const u8, fn() -> !>(thread_entry.as_ptr::<u8>())
        }))
        .unwrap();

        let mut thread = Thread {
            id: tid,
            task: thread_task,
        };

        let kernel_process = get_process(ProcessId::KERNEL_INIT_PID)
            .unwrap()
            .lock()
            .threads
            .insert_thread(tid);

        let thread = Arc::new(Mutex::new(thread));

        unsafe {
            THREAD_REGISTRY
                .get_unchecked()
                .write()
                .insert(tid, thread.clone());
        }

        thread
    }
}

#[derive(Debug)]
pub struct ThreadGroup {
    id: usize,
    threads: HashSet<ThreadId>,
}

impl ThreadGroup {
    pub fn new_empty() -> Self {
        let tg_id = FIRST_AVAILABLE_TGID.fetch_add(1, Ordering::Relaxed);

        Self {
            id: tg_id,
            threads: HashSet::new(),
        }
    }

    pub fn insert_thread(&mut self, thread_id: ThreadId) {
        self.threads.insert(thread_id);
    }
}
