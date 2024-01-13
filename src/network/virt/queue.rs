use core::slice;

pub struct VirtualQueue {
    pub size: u16,
    pub description: *mut VirtualQueueDescriptor,
    pub available: RingWrapper<VirtualQueueAvailable>,
    pub used: RingWrapper<VirtualQueueUsed>,
    pub last_used_idx: u16,
}

impl VirtualQueue {

    //unsafe fn new(size: u16, description: *mut VirtualQueueDescriptor) -> VirtualQueue {
    //    ?
    //    let used,
    //    let available,
    //    VirtualQueue {}
    //}

    pub fn mut_descriptors(&mut self) -> &mut [VirtualQueueDescriptor] {
        unsafe {
            slice::from_raw_parts_mut(self.description, self.size as usize)
        }
    }

    pub fn descriptors(&self) -> &[VirtualQueueDescriptor] {
        unsafe {
            slice::from_raw_parts(self.description, self.size as usize)
        }
    }
}

#[repr(C)]
pub struct VirtualQueueDescriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
pub struct VirtualQueueAvailable {
    pub flags : u16,
    pub idx : u16,
    pub ring : [u16; 0],
}

#[repr(C)]
pub struct VirtualQueueUsed {
    pub flags : u16,
    pub idx : u16,
    pub ring : [VirtualQueueUsedElement; 0],
}

#[repr(C)]
#[derive(Default, Clone)]
pub struct VirtualQueueUsedElement {
    pub id : u16,
    pub offset : u16,
    pub len : u32,
}

pub trait Ring {
    type Element;

    fn ring(&self) -> *const Self::Element;
    fn mut_ring(&mut self) -> *mut Self::Element;
}

pub struct RingWrapper<T: Ring> {
    ring: T,
    size: u16,
}

impl Ring for VirtualQueueAvailable {
    type Element = u16;

    fn ring(&self) -> *const u16 {
        self.ring.as_ptr()
    }

    fn mut_ring(&mut self) -> *mut u16 {
        self.ring.as_mut_ptr()
    }
}

impl Ring for VirtualQueueUsed {
    type Element = VirtualQueueUsedElement;

    fn ring(&self) -> *const VirtualQueueUsedElement {
        self.ring.as_ptr()
    }

    fn mut_ring(&mut self) -> *mut VirtualQueueUsedElement {
        self.ring.as_mut_ptr()
    }
}