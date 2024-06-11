//! x86 _Global Descriptor Table_ (`GDT`) related structures and methods.
//!
//! This table contains various `segment descriptors`, that provides the processor with the size and location of a
//! memory segment, along with various access controls and other specifiers.
//!
//! This is only useful in 16-bit `real mode` or in 32-bit `protected mode` if segmentation is used.

#![allow(clippy::as_conversions)]

use crate::errors::CanFail;
use crate::mem::{MemoryAddress, MemoryError, PhyAddr};
use crate::x86::msr::{Ia32ExtendedFeature, ModelSpecificRegister};
use crate::x86::privilege::PrivilegeLevel;
use crate::{error, BitIndex, Convertible};
use alloc::vec::Vec;
use alloc::{format, vec};
use core::arch::asm;
use core::fmt::{Debug, Display, Formatter};
use core::mem;
use core::mem::size_of;
use core::ptr::{read, NonNull};
use modular_bitfield::prelude::{B24, B4, B40, B5};
use modular_bitfield::specifiers::{B13, B2};
use modular_bitfield::{bitfield, BitfieldSpecifier};

pub const LONG_GDT_ADDR: u64 = 0x4000;

/// Initializes the long mode [`GlobalDescriptorTable`], with long code segment.
///
/// # Safety
///
/// Overwrites anything in memory at [`LONG_GDT_ADDR`].
#[allow(clippy::missing_panics_doc)]
pub unsafe fn long_init_gdt() {
    let mut gdt = GlobalDescriptorTable::new(PhyAddr::new(LONG_GDT_ADDR));
    gdt.add_entry::<DataSegmentType>(
        SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadWrite)
            .with_present(true)
            .with_base(PhyAddr::new(0))
            .unwrap()
            .with_limit(0xFF_FFF)
            .unwrap(),
    )
    .unwrap();
    gdt.add_entry::<CodeSegmentType>(
        SegmentDescriptor::new_segment::<CodeSegmentType>(CodeSegmentType::ExecuteRead)
            .with_present(true)
            .with_base(PhyAddr::new(0))
            .unwrap()
            .with_limit(0xFF_FFF)
            .unwrap()
            .enable_long()
            .unwrap(),
    )
    .unwrap();
    gdt.update();
}

/// _Global Descriptor Table_ (`GDT`) structure.
///
/// This table contains the `segment descriptors`, that contains information about memory segments, and are cached
/// by the processor when updating a segment register (`SS`, `CS`, `DS`, ...).
///
/// The first entry is ignored by the processor.
#[derive(Clone)]
pub struct GlobalDescriptorTable<A: MemoryAddress> {
    base_address: A,
    tail_address: A,
    length: u16,
    entries: Vec<SegmentDescriptor>,
}

impl<A: MemoryAddress> GlobalDescriptorTable<A> {
    /// Returns a new _Global Descriptor Table_, located at `base_address`.
    ///
    /// For a non 64-bit `GDT`, the content of the _GDTR_ is stored in the first entry, which is not possible for a
    /// 64-bit one as the _GDTR_ is larger than an entry.
    ///
    /// # Safety
    ///
    /// The given memory location must be free for use, and cannot be overwritten later on.
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn new(base_address: A) -> Self {
        let mut gdt = Self {
            base_address,
            tail_address: base_address + 0x8,
            length: 0,
            entries: vec![],
        };

        gdt.write_register();

        if size_of::<A::AsPrimitive>() == 8 {
            gdt.add_entry::<DataSegmentType>(SegmentDescriptor::new_segment(
                DataSegmentType::ReadOnly,
            ))
            .unwrap();
        }

        gdt
    }

    /// Loads the current _Global Descriptor Table_ currently in-use by the processor.
    ///
    /// # Safety
    ///
    /// The `GDT` pointed to by the _GDTR_ register must contain valid entries, otherwise it may cause UB.
    ///
    /// # Panics
    ///
    /// May also panic if the `GDT` pointed to by the _GDTR_ register contains invalid entries.
    #[must_use]
    pub unsafe fn load_gdt() -> Self {
        let gdtr: u128 = 0;

        asm!("sgdt [{}]", in(reg) &gdtr, options(nostack));
        let gdt_addr: A = A::AsPrimitive::try_from(gdtr >> 16)
            .unwrap_or_else(|_| panic!("invalid gdt address size"))
            .into();
        let gdt_length: u16 = gdtr.convert_with_mask(0xFFFF);

        let mut gdt_entries = vec![];
        let mut curr_pos = gdt_addr + 0x8;

        while curr_pos <= gdt_addr + usize::from(gdt_length) {
            let entry_inner = read(curr_pos.as_ptr::<SegmentDescriptorInner>());
            let mut entry = SegmentDescriptor::with_inner(entry_inner);

            if entry.is_system_segment() && A::WIDTH == 8 {
                curr_pos = curr_pos + 0x10;
            } else {
                let trunc_inner = SegmentDescriptorInner::from(u128::from(
                    Convertible::<u64>::low_bits(u128::from(entry_inner)),
                ));
                entry = SegmentDescriptor::with_inner(trunc_inner);
                curr_pos = curr_pos + 0x8;
            }
            if entry.present() {
                gdt_entries.push(entry);
            }
        }

        Self {
            base_address: gdt_addr,
            tail_address: gdt_addr + usize::from(gdt_length) + 1,
            length: gdt_length + 1,
            entries: gdt_entries,
        }
    }

    /// Updates the content of the _GDTR_ register, to point to this `Global Descriptor Table`.
    ///
    /// # Safety
    ///
    /// The _GDT_ must only contain valid entries. It is marked `unsafe` as it updates the CPU internals, and
    /// therefore may cause all sorts of undefined behaviour.
    unsafe fn update(&self) {
        asm!("lgdt [{}]", in(reg) self.base_address.as_ptr::<u8>(), options(nostack, nomem))
    }

    /// Updates the content of the _GDTR_ register, to match this `Global Descriptor Table`.
    ///
    /// # Safety
    ///
    /// The _GDT_ must only contain valid entries. It is marked `unsafe` as it updates the CPU internals, and therefore
    /// may cause all sorts of undefined behaviour.
    unsafe fn write_register(&mut self) {
        let gdt_address: NonNull<A::AsPrimitive> =
            self.base_address.as_nonnull_ptr().unwrap().byte_add(2);
        if size_of::<A::AsPrimitive>() == 8 {
            let gdt_base_address = self.base_address.add(0x8);
            gdt_address.write(gdt_base_address.into());
        } else {
            gdt_address.write(self.base_address.into());
            if self.length == 0 {
                self.length += 8;
            }
        }

        let gdt_size: NonNull<u16> = self.base_address.as_nonnull_ptr().unwrap();
        gdt_size.write(self.length - 1);
        self.update();
    }

    /// Adds a new `segment descriptor` (as a [`SegmentDescriptor`]) to this _Global Descriptor Table_.
    ///
    /// Updates the content of the _GDTR_ register as well.
    ///
    /// # Errors
    ///
    /// May return any variant of [`MemoryError`] if any of the memory addresses of the _GDT_ is invalid.
    ///
    /// # Safety
    ///
    /// Enough memory should be available after the current tail address of the `GDT`, and should not
    /// be overwritten later on.
    pub unsafe fn add_entry<T: SegmentType>(
        &mut self,
        descriptor: SegmentDescriptor,
    ) -> CanFail<MemoryError> {
        if A::WIDTH == 64 && descriptor.is_system_segment() {
            let descriptor_bytes = descriptor.as_long_mode_system_desc();
            self.tail_address.as_nonnull_ptr()?.write(descriptor_bytes);
            self.tail_address = self.tail_address + 0x10;
            self.length += 0x10;
        } else {
            let descriptor_bytes = descriptor.as_bytes();
            self.tail_address.as_nonnull_ptr()?.write(descriptor_bytes);
            self.tail_address = self.tail_address + 0x8;
            self.length += 0x8;
        }

        self.write_register();

        Ok(())
    }
}

impl<A: MemoryAddress> Debug for GlobalDescriptorTable<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("   Global Descriptor Table (GDT) : ")?;

        f.write_fmt(format_args!(
            "address = {} | length = {} | ",
            self.base_address, self.length
        ))?;

        if A::WIDTH == 8 {
            f.write_str("64-bit  ")?;
        } else {
            f.write_str("32-bit  ")?;
        }

        f.write_fmt(format_args!("\n{:->90}\n", ""))?;

        let mut gdt_entries = self.entries.iter();

        if let Some(first_entry) = gdt_entries.next() {
            f.write_fmt(format_args!("{first_entry:?}\n"))?;
            f.write_fmt(format_args!(
                "   raw -> {:#18x}",
                u128::from(first_entry.inner)
            ))?;
        }

        for entry in gdt_entries {
            f.write_fmt(format_args!("\n\n{entry}\n"))?;
            f.write_fmt(format_args!("   raw -> {:#18x}", u128::from(entry.inner)))?;
        }

        Ok(())
    }
}

impl<A: MemoryAddress> Display for GlobalDescriptorTable<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("   Global Descriptor Table (GDT) : ")?;

        if A::WIDTH == 8 {
            f.write_str("64-bit  ")?;
        } else {
            f.write_str("32-bit  ")?;
        }

        f.write_fmt(format_args!("\n{:->90}\n", ""))?;

        let mut gdt_entries = self.entries.iter();

        if let Some(first_entry) = gdt_entries.next() {
            f.write_fmt(format_args!("{first_entry:?}"))?
        }

        for entry in gdt_entries {
            f.write_fmt(format_args!("\n\n{entry}"))?;
        }

        Ok(())
    }
}

/// `Segment Descriptors` (_GDT_ entries) structure.
///
/// Contains information about memory segments, such as their size or location and access control specifiers.
/// These information are loaded into the segment registers when they are updated.
#[derive(Clone, Copy)]
pub struct SegmentDescriptor {
    inner: SegmentDescriptorInner,
}

impl Display for SegmentDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match (self.is_system_segment(), self.is_code_segment()) {
            (true, _) => f.write_fmt(format_args!(
                "SS {} | ",
                format!(
                    "{:^20}",
                    SystemSegmentType::from_byte(self.inner.access_byte_lo()).expect("invalid GDT")
                )
            )),
            (false, true) => f.write_fmt(format_args!(
                "CS {} | ",
                format!(
                    "{:^20}",
                    CodeSegmentType::from_byte(self.inner.access_byte_lo()).expect("invalid GDT")
                )
            )),
            (false, false) => f.write_fmt(format_args!(
                "DS {} | ",
                format!(
                    "{:^20}",
                    DataSegmentType::from_byte(self.inner.access_byte_lo()).expect("invalid GDT")
                )
            )),
        }?;

        f.write_fmt(format_args!("{:<18} | ", self.base()))?;
        f.write_fmt(format_args!("{:<12} | ", format!("{:#07x}", self.limit())))?;

        match (self.size(), self.long_mode()) {
            (true, _) => f.write_str("32-bit"),
            (false, false) => f.write_str("16-bit"),
            (false, true) => f.write_str("64-bit"),
        }?;

        f.write_fmt(format_args!(", {:?}", self.privilege_level()))
    }
}

impl Debug for SegmentDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str(&format!(
            "{:indent$}{:<22}{:<21}{:<15}{}\n{}",
            "",
            "Type",
            "Base",
            "Limit",
            "Flags",
            self,
            indent = 4
        ))?;

        Ok(())
    }
}

/// Internal bitfield associated to a [`SegmentDescriptor`].
#[bitfield]
#[derive(Clone, Copy, Debug)]
#[repr(u128)]
struct SegmentDescriptorInner {
    limit_lo: u16,
    base_lo: B24,
    access_byte_lo: B5,
    privilege_lvl: PrivilegeLevel,
    present: bool,
    limit_hi: B4,
    #[skip]
    __: bool,
    long_mode: bool,
    size: bool,
    granularity: bool,
    base_hi: B40,
    #[skip]
    __: u32,
}

impl SegmentDescriptor {
    /// Creates a new `segment descriptor` of the given type ([`SegmentType`]).
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadWrite);
    /// ```
    #[must_use]
    pub fn new_segment<T: SegmentType>(seg_type: T) -> Self {
        Self {
            inner: SegmentDescriptorInner::new(),
        }
        .with_segment_type(seg_type)
    }

    fn with_inner(inner: SegmentDescriptorInner) -> Self {
        Self { inner }
    }

    fn is_system_segment(&self) -> bool {
        self.inner.access_byte_lo().get_bit(4) == 0
    }

    fn is_code_segment(&self) -> bool {
        if !self.is_system_segment() {
            return self.inner.access_byte_lo().get_bit(3) != 0;
        }

        false
    }

    fn as_long_mode_system_desc(&self) -> u128 {
        u128::from(self.inner)
    }

    fn as_bytes(&self) -> u64 {
        u128::from(self.inner).low_bits()
    }

    /// Returns the segment limit field.
    ///
    /// That field is a 20-bit value used to specify the segment size. That value can be interpreted in two
    /// different ways depending on the value of the `granularity` flag.
    /// If that bit is clear, the limit is a number of bytes, and if set, a number of 4 KB pages. The segment size
    /// can therefore go as high as 4GB.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadWrite).with_limit(0xFF_FFF);
    /// assert_eq!(seg.limit(), 0xFF_FFF);
    /// ```
    pub(super) fn limit(&self) -> u32 {
        (u32::from(self.inner.limit_hi()) << 16) | u32::from(self.inner.limit_lo())
    }

    /// Returns a copy of this `segment descriptor` while updating the segment limit field.
    ///
    /// That field is a 20-bit value used to specify the segment size. That value can be interpreted in two
    /// different ways depending on the value of the `granularity` flag.
    /// If that bit is clear, the limit is a number of bytes, and if set, a number of 4 KB pages. The segment size
    /// can therefore go as high as 4GB.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadWrite).with_limit(0xFF_FFF);
    /// assert_eq!(seg.limit(), 0xFF_FFF);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`InvalidSegmentDescriptor::LimitTooLarge`] if the limit given as argument does not fit in 20 bits.
    pub fn with_limit(self, limit: u32) -> Result<Self, InvalidSegmentDescriptor> {
        if limit > 0xFF_FFF {
            return Err(InvalidSegmentDescriptor::LimitTooLarge);
        }

        Ok(Self::with_inner(
            self.inner
                .with_limit_lo(limit.convert_with_mask(0xFFFF))
                .with_limit_hi((limit >> 16).convert_with_mask(0xF)),
        ))
    }

    /// Returns the base address field for this `segment descriptor`.
    ///
    /// The value of that field indicates the location of the first byte of the segment inside the linear address
    /// space.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadWrite).with_base(0xFFFF_FFFF);
    /// assert_eq!(seg.base(), 0xFFFF_FFFF);
    /// ```
    #[must_use]
    pub fn base(&self) -> PhyAddr {
        PhyAddr::new((self.inner.base_hi() << 24) | u64::from(self.inner.base_lo()))
    }

    /// Returns a copy of this `segment descriptor` while updating the base address field value.
    ///
    /// The value of that field indicates the location of the first byte of the segment inside the linear address
    /// space. It usually is a 32-bit value, but may get extended to 64-bits for system registers in long mode.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadWrite).with_base(0xFFFF_FFFF);
    /// assert_eq!(seg.base(), 0xFFFF_FFFF);
    /// ```
    ///
    /// # Errors
    ///
    /// May return [`InvalidSegmentDescriptor::InvalidBaseAddress`] if the new base address is more than 32 bits wide
    /// for a non-system segment descriptor.
    pub fn with_base(self, address: PhyAddr) -> Result<Self, InvalidSegmentDescriptor> {
        if !self.is_system_segment() && address > PhyAddr::MAX_32 {
            return Err(InvalidSegmentDescriptor::InvalidBaseAddress);
        }

        Ok(Self::with_inner(
            self.inner
                .with_base_lo(u64::from(address).convert_with_mask(0x00FF_FFFF))
                .with_base_hi(u64::from(address) >> 24),
        ))
    }

    /// Returns a copy of this `segment descriptor` while updating the segment type.
    ///
    /// It indicates the segment type and the types of accesses it supports (read, read/write, execute only, ...).
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadOnly)
    ///                    .with_segment_type(DataSegmentType::ReadWrite);
    /// assert!(matches!(seg.segment_type(), DataSegmentType::ReadWrite));
    /// ```
    #[must_use]
    pub fn with_segment_type<T: SegmentType>(self, segment_type: T) -> Self {
        Self::with_inner(self.inner.with_access_byte_lo(segment_type.convert()))
    }

    /// Returns the value of the long mode flag for this `segment descriptor`.
    ///
    /// Used in `IA-32E` mode to indicate whether the code segment contains 64-bit native code or not.
    /// In any other mode or for non code segments, this bit must be clear.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<CodeSegmentType>(CodeSegmentType::ExecuteOnly);
    /// assert!(!seg.long_mode());
    /// ```
    #[must_use]
    pub fn long_mode(&self) -> bool {
        self.inner.long_mode()
    }

    /// Returns a copy of this `segment descriptor` while updating the value of the long mode flag.
    ///
    /// Used in `IA-32E` mode to indicate whether the code segment contains 64-bit native code or not.
    /// In any other mode or for non code segments, this bit must be clear.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<CodeSegmentType>(CodeSegmentType::ExecuteOnly);
    /// assert!(seg.enable_long().unwrap().long_mode());
    /// ```
    ///
    /// # Errors
    ///
    /// This flag can only be enabled for code segments, with the _D_ flag clear and in the `IA-32E` mode.
    /// Otherwise, may return [`InvalidSegmentDescriptor::NonCodeSegmentLongMode`],
    /// [`InvalidSegmentDescriptor::InvalidAddressSizeSpecifier`] or [`InvalidSegmentDescriptor::InvalidProcessorMode`]
    /// if any of the above conditions is not met.
    pub fn enable_long(self) -> Result<Self, InvalidSegmentDescriptor> {
        if !self.is_code_segment() {
            return Err(InvalidSegmentDescriptor::NonCodeSegmentLongMode);
        }

        if self.inner.size() {
            return Err(InvalidSegmentDescriptor::InvalidAddressSizeSpecifier);
        }

        if !Ia32ExtendedFeature::read()
            .ok_or(InvalidSegmentDescriptor::UnsupportedLongMode)?
            .ia32e_active()
        {
            return Err(InvalidSegmentDescriptor::InvalidProcessorMode);
        }

        Ok(Self::with_inner(self.inner.with_long_mode(true)))
    }

    /// Returns the value of the _present_ flag for this `segment register`.
    ///
    /// Indicates whether the segment is present in memory or not.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadOnly).with_present(true);
    /// assert!(seg.present());
    /// ```
    #[must_use]
    pub fn present(&self) -> bool {
        self.inner.present()
    }

    /// Returns a copy of this `segment descriptor` while updating the value of the present flag.
    ///
    /// Indicates whether the segment is present in memory or not.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadOnly).with_present(true);
    /// assert!(seg.present());
    /// ```
    #[must_use]
    pub fn with_present(self, is_present: bool) -> Self {
        Self::with_inner(self.inner.with_present(is_present))
    }

    /// Returns the value of the _descriptor privilege level_ field for this `segment descriptor`.
    ///
    /// It is used to control access to the segment.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadOnly)
    ///         .with_privilege_level(PrivilegeLevel::Ring1);
    /// assert!(matches!(seg.privilege_level(), PrivilegeLevel::Ring1));
    /// ```
    #[must_use]
    pub fn privilege_level(&self) -> PrivilegeLevel {
        self.inner.privilege_lvl()
    }

    /// Returns a copy of this `segment descriptor` while updating the value of the _descriptor privilege level_ field.
    ///
    /// It is used to control access to the segment.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadOnly)
    ///         .with_privilege_level(PrivilegeLevel::Ring1);
    /// assert!(matches!(seg.privilege_level(), PrivilegeLevel::Ring1));
    /// ```
    #[must_use]
    pub fn with_privilege_level(self, priv_lvl: PrivilegeLevel) -> Self {
        Self::with_inner(self.inner.with_privilege_lvl(priv_lvl))
    }

    /// Returns the value of the _granularity_ flag for this `segment descriptor`.
    ///
    /// Indicates the scaling of the _segment limit_ field, if clear that field shall be interpreted as a byte count,
    /// and when set, it indicates a number of 4KB pages.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadOnly).with_granularity(true);
    /// assert!(seg.granularity());
    /// ```
    #[must_use]
    pub fn granularity(&self) -> bool {
        self.inner.granularity()
    }

    /// Returns a copy of this `segment descriptor` while updating the value of the _granularity_ flag.
    ///
    /// Indicates the scaling of the _segment limit_ field, if clear that field shall be interpreted as a byte count,
    /// and when set, it indicates a number of 4KB pages.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<DataSegmentType>(DataSegmentType::ReadOnly).with_granularity(true);
    /// assert!(seg.granularity());
    /// ```
    #[must_use]
    pub fn with_granularity(self, granularity: bool) -> Self {
        Self::with_inner(self.inner.with_granularity(granularity))
    }

    /// Returns the value of the _D/B_ flag for this `segment descriptor`.
    ///
    /// Depending on the segment type, it can either indicate the default operation size (for code segments, it
    /// specifies the default length for effective address and operands referenced by instructions in this segment), or
    /// the size of the stack pointer used for implicit stack operations.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<CodeSegmentType>(CodeSegmentType::ExecuteOnly).with_size(true);
    /// assert!(seg.size());
    /// ```
    #[must_use]
    pub fn size(&self) -> bool {
        self.inner.size()
    }

    /// Returns a copy of this `segment descriptor` while updating the value of the _granularity_ flag.
    ///
    /// Depending on the segment type, it can either indicate the default operation size (for code segments, it
    /// specifies the default length for effective address and operands referenced by instructions in this segment), or
    /// the size of the stack pointer used for implicit stack operations.
    ///
    /// # Examples
    ///
    /// ```
    /// let seg = SegmentDescriptor::new_segment::<CodeSegmentType>(CodeSegmentType::ExecuteOnly).with_size(true);
    /// assert!(seg.size());
    /// ```
    #[must_use]
    pub fn with_size(self, size: bool) -> Self {
        Self::with_inner(self.inner.with_size(size))
    }
}

/// Represents a `segment descriptor` type (code, data, system).
///
/// Also indicates the kinds of accesses that can be made to that segment.
pub trait SegmentType: Sized {
    /// Converts the raw access bits specifying the segment type to a variant of `SegmentType`.
    fn from_byte(byte: u8) -> Option<Self>;

    /// Returns the raw access bits associated to this `SegmentType`.
    fn convert(self) -> u8;
}

/// A `GDT` entry may be a _code segment_ descriptor.
///
/// Code segments have 2 bits to specify the way it can be accessed: read enable and conforming bits.
/// They can be execute-only or execute read depending on the value of the read enable bit.
/// They can also be conforming or nonconforming. A transfer of execution into a more-privileged conforming
/// segment allows execution to continue at the current privilege level.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum CodeSegmentType {
    /// Execute-Only code segment.
    ExecuteOnly = 0b11000,

    /// Execute-Only code segment, accessed.
    ExecuteOnlyAccessed = 0b11001,

    /// Execute/Read code segment.
    ExecuteRead = 0b11010,

    /// Execute/Read code segment, accessed.
    ExecuteReadAccessed = 0b11011,

    /// Execute-Only code segment, conforming.
    ExecuteOnlyConforming = 0b11100,

    /// Execute-Only code segment, conforming and accessed.
    ExecuteOnlyConformingAccessed = 0b11101,

    /// Execute/Read code segment, conforming.
    ExecuteReadConforming = 0b11110,

    /// Execute/Read code segment, conforming and accessed.
    ExecuteReadConformingAccessed = 0b1111,
}

impl Display for CodeSegmentType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(&format!("{self:?}"))
    }
}

impl SegmentType for CodeSegmentType {
    fn from_byte(byte: u8) -> Option<CodeSegmentType> {
        if byte.get_bit(3) != 1 {
            return None;
        }
        Some(unsafe { mem::transmute(byte) })
    }

    fn convert(self) -> u8 {
        match self {
            CodeSegmentType::ExecuteOnly => 0b11000,
            CodeSegmentType::ExecuteOnlyAccessed => 0b11001,
            CodeSegmentType::ExecuteRead => 0b11010,
            CodeSegmentType::ExecuteReadAccessed => 0b11011,
            CodeSegmentType::ExecuteOnlyConforming => 0b11100,
            CodeSegmentType::ExecuteOnlyConformingAccessed => 0b11101,
            CodeSegmentType::ExecuteReadConforming => 0b11110,
            CodeSegmentType::ExecuteReadConformingAccessed => 0b11111,
        }
    }
}

/// A `GDT` entry may be a _data segment_ descriptor.
///
/// Data segments have 2 bits to specify how it can be accessed and used: write enable and expansion direction.
/// They can be read-only or read/write, depending on the value of the write-enable bit. If the size of a stack
/// segment has to be changed dynamically, the stack segment can be an expand-down data segment, if the expansion
/// direction flag is set.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum DataSegmentType {
    /// Read-Only data segment.
    ReadOnly = 0b10000,

    /// Read-Only data segment, accessed.
    ReadOnlyAccessed = 0b10001,

    /// Read/Write data segment.
    ReadWrite = 0b10010,

    /// Read/Write data segment, accessed.
    ReadWriteAccessed = 0b10011,

    /// Read-Only data segment, expand down.
    ReadOnlyExpandDown = 0b10100,

    /// Read-Only data segment, expand down and accessed.
    ReadOnlyExpandDownAccessed = 0b10101,

    /// Read-Write data segment, expand down.
    ReadWriteExpandDown = 0b10110,

    /// Read-Write data segment, expand down and accessed.
    ReadWriteExpandDownAccessed = 0b10111,
}

impl Display for DataSegmentType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(&format!("{self:?}"))
    }
}

impl SegmentType for DataSegmentType {
    fn from_byte(byte: u8) -> Option<Self> {
        if byte.get_bit(3) != 0 {
            return None;
        }
        Some(unsafe { mem::transmute(byte) })
    }

    fn convert(self) -> u8 {
        match self {
            DataSegmentType::ReadOnly => 0b10000,
            DataSegmentType::ReadOnlyAccessed => 0b10001,
            DataSegmentType::ReadWrite => 0b10010,
            DataSegmentType::ReadWriteAccessed => 0b10011,
            DataSegmentType::ReadOnlyExpandDown => 0b10100,
            DataSegmentType::ReadOnlyExpandDownAccessed => 0b10101,
            DataSegmentType::ReadWriteExpandDown => 0b10110,
            DataSegmentType::ReadWriteExpandDownAccessed => 0b10111,
        }
    }
}

/// A `GDT` entry may be a _system_ descriptor, when the corresponding bit is clear.
///
/// There is two categories of system descriptors:
///
/// - `system-segment` descriptors: points to a system segment (`LDT` or `TSS` segments).
///
/// - `gate descriptors`: hold pointers to procedure entry points in code segments (_call_,
///   _interrupt_ or _trap_ gates), or segment selectors for `TSS` (_task_ gate).
#[derive(BitfieldSpecifier, Clone, Copy, Debug)]
#[bits = 5]
#[repr(u8)]
pub enum SystemSegmentType {
    /// 16-bit available `Task-state segment` descriptor.
    AvailableTSS16 = 0b00001,

    /// `Local descriptor-table` descriptor.
    LDT = 0b00010,

    /// 16-bit occupied `Task-state segment` descriptor.
    BusyTSS16 = 0b00011,

    /// 16-bit `Call-gate` descriptor.
    CallGate16 = 0b00100,

    /// 32/64-bit `Task-gate` descriptor.
    TaskGate = 0b00101,

    /// 16-bit `Interrupt-gate` descriptor.
    InterruptGate16 = 0b00110,

    /// 16-bit `Trap-gate` descriptor.
    TrapGate16 = 0b00111,

    /// 32/64-bit available `Task-state segment` descriptor.
    AvailableTSS = 0b01001,

    /// 32/64-bit busy `Task-state segment` descriptor.
    BusyTSS = 0b01011,

    /// 32/64-bit `Call-gate` descriptor.
    CallGate = 0b01100,

    /// 32/64-bit `Interrupt-gate` descriptor.
    InterruptGate = 0b01110,

    /// 32/64-bit `Trap-gate` descriptor.
    TrapGate = 0b01111,
}

impl Display for SystemSegmentType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(&format!("{self:?}"))
    }
}

impl SegmentType for SystemSegmentType {
    fn from_byte(byte: u8) -> Option<Self> {
        if byte == 0 {
            return None;
        }
        Some(unsafe { mem::transmute(byte) })
    }

    fn convert(self) -> u8 {
        match self {
            SystemSegmentType::AvailableTSS16 => 0b00001,
            SystemSegmentType::LDT => 0b00010,
            SystemSegmentType::BusyTSS16 => 0b00011,
            SystemSegmentType::CallGate16 => 0b00100,
            SystemSegmentType::TaskGate => 0b00101,
            SystemSegmentType::InterruptGate16 => 0b00110,
            SystemSegmentType::TrapGate16 => 0b00111,
            SystemSegmentType::AvailableTSS => 0b01001,
            SystemSegmentType::BusyTSS => 0b01011,
            SystemSegmentType::CallGate => 0b01100,
            SystemSegmentType::InterruptGate => 0b01110,
            SystemSegmentType::TrapGate => 0b01111,
        }
    }
}

pub struct SegmentSelector {
    pub(super) inner: SegmentSelectorInner,
}

#[bitfield]
#[derive(BitfieldSpecifier)]
#[repr(u16)]
pub(super) struct SegmentSelectorInner {
    /// Requested privilege level of the selector ([`x86::privilege::PrivilegeLevel`]).
    ///
    /// Determines if the selector is valid during permission checks and may set execution or memoy access privilege.
    rpl: B2,

    /// Specifies which descriptor table to use, if clear the `GDT` is used, otherwise the `LDT` is used.
    ti: bool,

    /// Specifies the index of the `GDT` or `LDT` entry referenced by this _selector_.   
    index: B13,
}

impl SegmentSelector {
    pub fn with_rpl(self, rpl: PrivilegeLevel) -> Self {
        let rpl_bits = match rpl {
            PrivilegeLevel::Ring0 => 0b00,
            PrivilegeLevel::Ring1 => 0b01,
            PrivilegeLevel::Ring2 => 0b10,
            PrivilegeLevel::Ring3 => 0b11,
        };

        Self {
            inner: self.inner.with_rpl(rpl_bits),
        }
    }

    pub fn gdt_selector() -> Self {
        Self {
            inner: SegmentSelectorInner::new(),
        }
    }

    pub fn ldt_selector() -> Self {
        Self {
            inner: SegmentSelectorInner::new().with_ti(true),
        }
    }

    pub fn with_index(self, index: u16) -> Result<Self, InvalidSegmentSelector> {
        if index & 0b111 != 0 {
            return Err(InvalidSegmentSelector::InvalidIndexAlignment);
        }

        Ok(Self {
            inner: self.inner.with_index(index),
        })
    }
}

pub enum InvalidSegmentSelector {
    InvalidIndexAlignment,
}

/// Error type when building a [`SegmentDescriptor`].
#[derive(Clone, Copy, Debug)]
pub enum InvalidSegmentDescriptor {
    /// Invalid value given for the base address field (64-bit for non system-segment, ...).
    InvalidBaseAddress,

    /// Invalid value given for the limit field (it does not fit in 20 bits).
    LimitTooLarge,

    /// Tried to enable long mode with the `D/B` flag set.
    InvalidAddressSizeSpecifier,

    /// Tried to enable long mode on a non-code segment.
    NonCodeSegmentLongMode,

    /// Long mode is not available on this system.
    UnsupportedLongMode,

    /// Tried to enable long mode without `IA-32E` enabled.
    InvalidProcessorMode,
}
