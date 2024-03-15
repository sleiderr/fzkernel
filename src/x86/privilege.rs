//! _x86 Privilege Levels_, part of the segment-protection mechanism.
//!
//! The processor prevents tasks operating at a lower privilege level to access segments with a greater privilege.
//! There exists three different types of privilege level:
//!
//! - _Current privilege level_ (`CPL`): privilege level of the currently executing task. It is usually equal to the
//! privilege level of the `code segment` from which the instructions are being fetched. The processor changes the `CPL`
//! when switching control to a code segment with a different privilege level, except for conforming code segments.
//!
//! - _Descriptor privilege level_ (`DPL`): privilege level of a segment or gate. It is stored in the `DPL` field of
//! the segment or gate descriptor. When the currently executing code segment attempts to access a segment or gate,
//! the `DPL` of the segment gets compared to the `CPL` and `RPL` of the segment or gate selector. The `DPL` may be
//! interpreted differently, depending on the type of segment or gate being accessed.
//!
//! - _Requested privilege level_ (`RPL`): override privilege assigned to segment selectors. It can be used to ensure
//! that privileged code does not access a segment on behalf of an application program unless that program has itself
//! access to that segment (by setting the `RPL` to the `CPL` of the program).

#![allow(clippy::as_conversions)]

use modular_bitfield::BitfieldSpecifier;

/// Available privilege levels.
///
/// The processor recognizes four different privilege levels (`rings`). The center ring (`Ring0`) is used for the most
/// privileged code (usually kernel code) as it is the one with the most access rights. Outer rings are used for
/// OS services or userland code.
#[derive(BitfieldSpecifier, Copy, Clone, Debug)]
#[repr(u8)]
pub enum PrivilegeLevel {
    /// `Ring 0` privilege level.
    ///
    /// It is the most privileged ring, and therefore should be use for kernel code only.
    Ring0,

    /// `Ring 1` privilege level.
    Ring1,

    /// `Ring 2` privilege level.
    Ring2,

    /// `Ring 3` privilege level.
    ///
    /// Less privileged ring, usually used for userland applications.
    Ring3,
}
