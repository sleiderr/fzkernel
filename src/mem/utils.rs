//! Bitwise operations and other utilities for memory management and related.

use core::ops::{Add, BitAnd, Shl, Shr, Sub};

/// Used to convert between integer types of different bit types, by masking bits we want to get rid of.
pub trait Convertible<T: TryFrom<Self>>: Sized + BitAnd<Output = Self> {
    /// Mask corresponding to the low-order bits of the number.
    const LOW_BITS_MASK: Self;

    /// Mask corresponding to the high-order bits of the number.
    const HIGH_BITS_MASK: Self;

    /// Converts between integer types, by masking bits.
    ///
    /// # Panics
    ///
    /// Will panic if the resulting number after masking does not fit in a number of type `T`.
    fn convert_with_mask(self, mask: Self) -> T {
        T::try_from(self & mask).ok().unwrap()
    }

    /// Returns the high-order bits of this number.
    fn high_bits(self) -> T {
        self.convert_with_mask(Self::HIGH_BITS_MASK)
    }

    /// Returns the low-order bits of this number.
    fn low_bits(self) -> T {
        self.convert_with_mask(Self::LOW_BITS_MASK)
    }
}

impl Convertible<u64> for u128 {
    const LOW_BITS_MASK: Self = 0xFFFF_FFFF_FFFF_FFFF;
    const HIGH_BITS_MASK: Self = 0xFFFF_FFFF_FFFF_FFFF << 64;
}

impl Convertible<u16> for u128 {
    const LOW_BITS_MASK: Self = 0xFFFF;
    const HIGH_BITS_MASK: Self = 0xFFFF << 112;
}

impl Convertible<u32> for u64 {
    const LOW_BITS_MASK: Self = 0xFFFF_FFFF;
    const HIGH_BITS_MASK: Self = 0xFFFF_FFFF_0000_0000;
}

impl Convertible<u16> for u64 {
    const LOW_BITS_MASK: Self = 0xFFFF;
    const HIGH_BITS_MASK: Self = 0xFFFF_0000_0000_0000;
}

impl Convertible<u16> for u32 {
    const LOW_BITS_MASK: Self = 0xFFFF;
    const HIGH_BITS_MASK: Self = 0xFFFF_0000;
}

impl Convertible<u8> for u32 {
    const LOW_BITS_MASK: Self = 0xFF;
    const HIGH_BITS_MASK: Self = 0xFF00_0000;
}

/// Used to access individual bits of a number.
pub trait BitIndex:
    Copy
    + Sized
    + Add<Output = Self>
    + Shr<Output = Self>
    + Shl<Output = Self>
    + BitAnd<Output = Self>
    + Sub<Output = Self>
{
    /// Multiplicative identity for that number type.
    const IDENT: Self;

    /// Returns the value bit placed at the given index.
    #[must_use]
    fn get_bit(&self, index: Self) -> Self {
        (*self & (Self::IDENT << index)) >> index
    }

    /// Returns a bit slice between two given indexes.
    #[must_use]
    fn get_bit_slice(&self, first_bit: Self, last_bit: Self) -> Self {
        (*self >> first_bit) & ((Self::IDENT << (Self::IDENT + last_bit - first_bit)) - Self::IDENT)
    }
}

impl BitIndex for u128 {
    const IDENT: Self = 1;
}

impl BitIndex for u64 {
    const IDENT: Self = 1;
}

impl BitIndex for u32 {
    const IDENT: Self = 1;
}

impl BitIndex for u8 {
    const IDENT: Self = 1;
}
