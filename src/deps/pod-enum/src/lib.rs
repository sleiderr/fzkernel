//! Make an enum which can act like plain-old-data
//!
//! See [`pod_enum`] for details.

#![no_std]

#[doc(hidden)]
pub use bytemuck;

/// Make a plain-old-data enum
///
/// This macro changes the enum to allow for "unknown" variants which take the discriminant
/// values not listed in the definition, thus allowing the enum to be valid for any bit pattern
/// and have no uninitialized bytes. We also implement [`bytemuck::Pod`] for the enum, since it
/// meets those guarantees.
///
/// The representation of the enum must be specified as a type (any of the `repr(u*)` or
/// `repr(i*)` types are supported, not `repr(C)` or `repr(Rust)`). The enum will have the same
/// size and alignment as the given representation.
///
/// The resulting type also gets implementations of [`Debug`](core::fmt::Debug) and
/// [`PartialEq`].
///
/// # Example
/// ```
/// use pod_enum::pod_enum;
///
/// /// An example enum
/// #[pod_enum]
/// #[repr(u8)]
/// enum Foo {
///     /// Bar
///     Bar = 0,
///     Bat = 1,
///     Baz = 2,
/// }
///
/// assert_eq!(Foo::from(0), Foo::Bar);
/// assert_eq!(u8::from(Foo::Bat), 1);
/// assert_eq!(&format!("{:?}", Foo::Baz), "Baz");
/// assert_eq!(&format!("{:?}", Foo::from(3)), "Unknown (3)");
/// ```
///
/// Note that all variants of the enum must have an explicitly annotated discriminant, and no
/// fields may be listed on any of the variants.
///
/// # Safety
/// This macro checks that the internal representation used is [`bytemuck::Pod`], and will
/// return an error otherwise. There is no risk of unsound code being generated.
/// ```compile_fail
/// use core::num::NonZeroU8;
/// use pod_enum::pod_enum;
///
/// #[pod_enum]
/// // This is not allowed - `bool` has invalid bit patterns
/// #[repr(bool)]
/// enum Foo {
///     Bar = false,
/// }
/// ```
pub use pod_enum_macros::pod_enum;

/// A trait marking this type as a plain-old-data enum
///
/// This is intended to be implemented by the [`pod_enum`] attribute macro, not to be
/// implemented on its own.
pub trait PodEnum: bytemuck::Pod {
    /// The type used for the internal representation
    type Repr: bytemuck::Pod;
}
