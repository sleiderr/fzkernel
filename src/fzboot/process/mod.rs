use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use conquer_once::spin::OnceCell;
use hashbrown::HashMap;
use spin::{Mutex, RwLock};
use thread::{Thread, ThreadGroup, ThreadId, THREAD_REGISTRY};

use crate::{
    kernel_syms::{KERNEL_PAGE_TABLE, PAGE_SIZE},
    mem::{MemoryAddress, PhyAddr, VirtAddr},
    x86::paging::{page_alloc::frame_alloc::alloc_page, PageTable},
};

use super::scheduler::task::{get_task, TaskId};

pub mod thread;

static FIRST_AVAILABLE_PID: AtomicUsize = AtomicUsize::new(1);
static PROCESS_REGISTRY: OnceCell<RwLock<BTreeMap<ProcessId, Arc<Mutex<Process>>>>> =
    OnceCell::uninit();

pub fn init_kernel_process() {
    PROCESS_REGISTRY.init_once(|| RwLock::new(BTreeMap::new()));

    let mut kernel_process = Process {
        id: ProcessId::KERNEL_INIT_PID,
        name: String::from("system"),
        threads: ThreadGroup::new_empty(),
        parent: None,
        page_table: PhyAddr::NULL_PTR,
        flags: ProcessFlags::default(),
    };

    kernel_process.page_table = KERNEL_PAGE_TABLE;

    kernel_process
        .threads
        .insert_thread(ThreadId::KERNEL_INIT_TID);

    let kernel_init_task = get_task(TaskId::INIT_TASK).unwrap();

    kernel_init_task.lock().pid = ProcessId::KERNEL_INIT_PID;

    let mut kernel_thread = Arc::new(Mutex::new(Thread {
        id: ThreadId::KERNEL_INIT_TID,
        task: kernel_init_task,
    }));

    THREAD_REGISTRY.init_once(|| RwLock::new(HashMap::new()));

    unsafe {
        THREAD_REGISTRY
            .get_unchecked()
            .write()
            .insert(ThreadId::KERNEL_INIT_TID, kernel_thread);
    }

    let kernel_process = Arc::new(Mutex::new(kernel_process));

    unsafe {
        PROCESS_REGISTRY
            .get_unchecked()
            .write()
            .insert(ProcessId::KERNEL_INIT_PID, kernel_process);
    }
}

pub fn get_process(process_id: ProcessId) -> Option<Arc<Mutex<Process>>> {
    unsafe {
        PROCESS_REGISTRY
            .get_unchecked()
            .read()
            .get(&process_id)
            .cloned()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(usize);

impl ProcessId {
    pub const KERNEL_INIT_PID: Self = Self(0);
}

#[derive(Debug)]
pub struct Process {
    id: ProcessId,
    name: String,
    threads: ThreadGroup,
    parent: Option<ProcessId>,
    page_table: PhyAddr,
    flags: ProcessFlags,
}

impl Process {
    pub fn spawn_process(
        process_entry: VirtAddr,
        flags: ProcessFlags,
    ) -> Result<ProcessId, ProcessCreationError> {
        let pid = ProcessId(FIRST_AVAILABLE_PID.fetch_add(1, Ordering::Relaxed));

        let process_page_table_addr = alloc_page(PAGE_SIZE)
            .map_err(|_| ProcessCreationError::MemoryAllocationError)?
            .start;

        unsafe {
            *process_page_table_addr.as_mut_ptr::<PageTable>() = PageTable::create_process_table();
        }

        let process = Arc::new(Mutex::new(Process {
            id: pid,
            name: String::default(),
            threads: ThreadGroup::new_empty(),
            parent: None,
            page_table: process_page_table_addr,
            flags: flags,
        }));

        unsafe {
            PROCESS_REGISTRY
                .get_unchecked()
                .write()
                .insert(pid, process.clone());
        }

        let mut process = process.lock();

        Ok(pid)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessFlags(u64);

impl Default for ProcessFlags {
    fn default() -> Self {
        Self(0)
    }
}

pub enum ProcessCreationError {
    MemoryAllocationError,
}
