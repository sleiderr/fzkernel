pub struct VirtualQueue {
    size: u16,
    description: *mut VirtualQueueDescriptor,
    available: RingWrapper<VirtualQueueAvailable>,
    used: RingWrapper<VirtualQueueUsed>,
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