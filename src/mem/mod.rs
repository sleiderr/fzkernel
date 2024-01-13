//! Memory related utilities module.

use bytemuck::{Pod, Zeroable};
use core::cell::UnsafeCell;
use core::ops::{Add, AddAssign, BitAnd, Shr};
use core::ptr;

use conquer_once::spin::OnceCell;

pub mod bmalloc;
pub mod e820;
pub mod gdt;

pub static MEM_STRUCTURE: OnceCell<MemoryStructure> = OnceCell::uninit();

pub struct LocklessCell<T> {
    data: UnsafeCell<T>,
}

impl<T> LocklessCell<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }

    pub fn get(&self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }
}

unsafe impl<T> Send for LocklessCell<T> {}
unsafe impl<T> Sync for LocklessCell<T> {}

#[derive(Clone, Copy, Debug)]
pub struct Alignment(u64);

impl Alignment {
    pub const ALIGN_4KB: Self = Self(1 << 12);
}

#[derive(Clone, Copy, Debug)]
pub struct VirtAddr(u64);

impl VirtAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr % (1 << 48))
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.0 as *mut T
    }

    pub const fn to_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    pub fn is_aligned_with(&self, align: Alignment) -> bool {
        self.0 % align.0 == 0
    }
}

impl Add for VirtAddr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        VirtAddr::new(self.0 + rhs.0)
    }
}

impl From<VirtAddr> for u64 {
    fn from(value: VirtAddr) -> Self {
        value.0
    }
}

impl From<PhyAddr> for u64 {
    fn from(value: PhyAddr) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PhyAddr(u64);

impl PhyAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr % (1 << 52))
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.0 as *mut T
    }

    pub fn is_aligned_with(&self, align: Alignment) -> bool {
        self.0 % align.0 == 0
    }
}

impl Add<u64> for PhyAddr {
    type Output = PhyAddr;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl<T> From<*mut T> for PhyAddr {
    fn from(value: *mut T) -> Self {
        Self(value as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Pod, Zeroable)]
#[repr(transparent)]
pub struct PhyAddr32(u32);

impl PhyAddr32 {
    pub const fn new(addr: u32) -> Self {
        Self(addr)
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.0 as *mut T
    }

    pub fn is_aligned_with(&self, align: Alignment) -> bool {
        self.0 % u32::try_from(align.0).unwrap() == 0
    }
}

impl From<u32> for PhyAddr32 {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<PhyAddr32> for u32 {
    fn from(value: PhyAddr32) -> Self {
        value.0
    }
}

impl AddAssign<u32> for PhyAddr32 {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs;
    }
}

impl Add<u32> for PhyAddr32 {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0.saturating_add(rhs))
    }
}

impl BitAnd<u32> for PhyAddr32 {
    type Output = u32;

    fn bitand(self, rhs: u32) -> Self::Output {
        self.0 & rhs
    }
}

impl Shr<u32> for PhyAddr32 {
    type Output = u32;

    fn shr(self, rhs: u32) -> Self::Output {
        self.0 >> rhs
    }
}

pub struct MemoryStructure {
    pub heap_addr: usize,
    pub heap_size: usize,
}

/// Zeroise the .bss segment when entering the program.
///
/// Uses two external symbols `_bss_start` and `_bss_end` that must
/// be added at link time and that points to the start and the end
/// of the .bss section respectively.
pub fn zero_bss() {
    extern "C" {
        static _bss_start: u8;
        static _bss_end: u8;
    }

    unsafe {
        let bss_start = &_bss_start as *const u8 as u32;
        let bss_end = &_bss_end as *const u8 as u32;
        let bss_len = bss_end - bss_start;

        ptr::write_bytes(bss_start as *mut u8, 0, bss_len as usize);
    }
}
