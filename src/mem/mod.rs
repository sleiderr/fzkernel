//! Memory related utilities module.

use alloc::format;
use bytemuck::{Pod, Zeroable};
use core::cell::UnsafeCell;
use core::fmt::{Display, Formatter};
use core::ops::{Add, AddAssign, BitAnd, Rem, Shr, Sub};
use core::ptr;
use core::ptr::NonNull;

use conquer_once::spin::OnceCell;

use crate::x86::paging::page_table::mapper::{MemoryMapping, PhysicalMemoryMapping};

pub mod bmalloc;
pub mod e820;
pub mod kernel_sec;
pub mod stack;
pub mod utils;
#[cfg(feature = "x86_64")]
pub mod vmalloc;

pub static MEM_STRUCTURE: OnceCell<MemoryStructure> = OnceCell::uninit();

/// Returns a pointer to the physical memory located at address `addr`;
///
/// Uses the current physical memory mappings to make direct memory access, by converting the given physical address into
/// a virtual address than can then be used to access the underlying physical memory.
///
/// # Examples
///
/// ```
/// let idt_ptr = get_physical_memory(PhyAddr::new(0x0));
/// ```
#[inline]
pub fn get_physical_memory(addr: PhyAddr) -> *mut u8 {
    get_physical_memory_mapping().convert(addr).as_mut_ptr()
}

/// Returns a pointer to the physical memory located at address `addr`;
///
/// Uses the current physical memory mappings to make direct memory access, by converting the given physical address into
/// a virtual address than can then be used to access the underlying physical memory.
///
/// # Examples
///
/// ```
/// let idt_ptr = get_physical_memory(PhyAddr::new(0x0));
/// ```
#[inline(always)]
pub fn get_physical_memory32(addr: PhyAddr32) -> *mut u8 {
    get_physical_memory_mapping()
        .convert(PhyAddr::from(addr))
        .as_mut_ptr()
}

#[inline(always)]
fn get_physical_memory_mapping() -> PhysicalMemoryMapping {
    #[cfg(feature = "x86_64")]
    return PhysicalMemoryMapping::KERNEL_DEFAULT_MAPPING;

    #[cfg(not(feature = "x86_64"))]
    return PhysicalMemoryMapping::IDENTITY;
}

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

impl TryFrom<Alignment> for u32 {
    type Error = MemoryError;

    fn try_from(value: Alignment) -> Result<Self, Self::Error> {
        u32::try_from(value.0).map_err(|_| MemoryError::InvalidAlignment)
    }
}

impl From<u32> for Alignment {
    fn from(value: u32) -> Self {
        Self(u64::from(value))
    }
}

impl From<u64> for Alignment {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl TryFrom<Alignment> for u64 {
    type Error = MemoryError;

    fn try_from(value: Alignment) -> Result<Self, Self::Error> {
        Ok(value.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct VirtAddr(u64);

impl VirtAddr {
    pub const fn new(addr: u64) -> Self {
        let sg_extd = (addr & (1 << 47)) >> 47;

        Self(addr | sg_extd * (0xFFFF << 48))
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

impl Add<usize> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        VirtAddr::new(self.0 + u64::try_from(rhs).expect("infaillible conversion"))
    }
}

impl Add<u64> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        VirtAddr::new(self.0 + rhs)
    }
}

impl Sub<usize> for VirtAddr {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        VirtAddr::new(self.0 - u64::try_from(rhs).expect("infaillible conversion"))
    }
}

impl Sub<u64> for VirtAddr {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self::Output {
        VirtAddr::new(self.0 - rhs)
    }
}

impl Add for VirtAddr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        VirtAddr::new(self.0 + rhs.0)
    }
}

impl From<u64> for VirtAddr {
    fn from(value: u64) -> Self {
        VirtAddr::new(value)
    }
}

impl From<VirtAddr> for u64 {
    fn from(value: VirtAddr) -> Self {
        let sg_extd = (value.0 & (1 << 47)) >> 47;

        value.0 | sg_extd * (0xFFFF << 48)
    }
}

impl From<PhyAddr> for u64 {
    fn from(value: PhyAddr) -> Self {
        value.0
    }
}

impl Display for VirtAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(&format!("{:#018x}", self.0))
    }
}

#[derive(Clone, Copy, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PhyAddr(u64);

impl PhyAddr {
    pub const MAX_32: Self = Self(0xFFFF_FFFF);

    pub const fn new(addr: u64) -> Self {
        Self(addr % (1 << 52))
    }

    pub const fn const_mut_convert<T>(&self) -> *mut T {
        self.0 as *mut T
    }
}

impl Display for PhyAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(&format!("{:#018x}", self.0))
    }
}

impl MemoryAddress for PhyAddr {
    const WIDTH: u64 = 8;
    const NULL_PTR: Self = Self(0);

    type AsPrimitive = u64;

    fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl From<u64> for PhyAddr {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<PhyAddr32> for PhyAddr {
    fn from(value: PhyAddr32) -> Self {
        Self::new(value.0.into())
    }
}

impl Add<usize> for PhyAddr {
    type Output = PhyAddr;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + u64::try_from(rhs).expect("invalid offset"))
    }
}

impl Add<u64> for PhyAddr {
    type Output = PhyAddr;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Add<PhyAddr> for PhyAddr {
    type Output = PhyAddr;

    fn add(self, rhs: PhyAddr) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl<T> From<*mut T> for PhyAddr {
    fn from(value: *mut T) -> Self {
        Self(value as u64)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd, Pod, Zeroable)]
#[repr(transparent)]
pub struct PhyAddr32(u32);

impl PhyAddr32 {
    pub const fn new(addr: u32) -> Self {
        Self(addr)
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

impl From<PhyAddr32> for u64 {
    fn from(value: PhyAddr32) -> Self {
        u64::from(value.0)
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

impl Add<usize> for PhyAddr32 {
    type Output = PhyAddr32;

    fn add(self, rhs: usize) -> Self::Output {
        Self(
            self.0
                .saturating_add(u32::try_from(rhs).expect("invalid address")),
        )
    }
}

impl Display for PhyAddr32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(&format!("{:#010x}", self.0))
    }
}

pub trait MemoryAddress:
    Display + Sized + Clone + Copy + Add<usize, Output = Self> + Into<u64> + PartialEq + PartialOrd
{
    const WIDTH: u64;
    const NULL_PTR: Self;

    type AsPrimitive: Into<Self>
        + From<Self>
        + TryFrom<u128>
        + TryFrom<Alignment, Error = MemoryError>
        + Rem<Output = Self::AsPrimitive>;

    fn as_ptr<T>(&self) -> *const T;

    fn as_mut_ptr<T>(&self) -> *mut T;

    fn as_nonnull_ptr<T>(&self) -> Result<NonNull<T>, MemoryError> {
        NonNull::new(self.as_mut_ptr()).ok_or(MemoryError::NullPointer)
    }

    fn is_null(&self) -> bool;

    fn is_aligned_with(&self, align: Alignment) -> Result<bool, MemoryError> {
        Ok(
            Into::<Self>::into(
                Self::AsPrimitive::from(*self) % Self::AsPrimitive::try_from(align)?,
            )
            .is_null(),
        )
    }
}

impl MemoryAddress for PhyAddr32 {
    const WIDTH: u64 = 4;
    const NULL_PTR: Self = Self(0x0);

    type AsPrimitive = u32;

    fn as_ptr<T>(&self) -> *const T {
        get_physical_memory32(*self) as *const T
    }

    fn as_mut_ptr<T>(&self) -> *mut T {
        get_physical_memory32(*self) as *mut T
    }

    fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl MemoryAddress for VirtAddr {
    const WIDTH: u64 = 8;
    const NULL_PTR: Self = Self(0x0);

    type AsPrimitive = u64;

    fn as_ptr<T>(&self) -> *const T {
        Self::AsPrimitive::from(*self) as *const T
    }

    fn as_mut_ptr<T>(&self) -> *mut T {
        Self::AsPrimitive::from(*self) as *mut T
    }

    fn is_null(&self) -> bool {
        self.0 == 0
    }
}

pub struct MemoryStructure {
    pub heap_addr: usize,
    pub heap_size: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum MemoryError {
    InvalidAlignment,
    NullPointer,
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
