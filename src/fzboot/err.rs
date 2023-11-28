use core::{fmt::Debug, str::Utf8Error};

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::collections::TryReserveError;

/// `BaseError` is a common trait implemented by every error type defined in FrozenBoot.
///
/// It is dependent on the [`Debug`] trait, which makes sense as we are dealing with errors.
/// [`GenericError`] and general `Exception` are defined using this trait when paired with a global
/// allocator (required for trait objects).
pub trait BaseError: Debug {}

/// `CanFail` is a return type for functions that are allowed to fail, and don't need to return
/// anything.
///
/// For instance, it could be used when checking if a functionnality / feature is available on the
/// system, or when initializing a component, or a shared `static`.
///
/// # Examples:
///
/// ```
/// use fzboot::CanFail;
///
/// struct InitError {
///     error: String
/// }
///
/// fn init_component() -> CanFail<InitError> {
///     todo!()
/// }
/// ```
pub type CanFail<T> = Result<(), T>;

/// `GenericError` is a return type for functions that do not raise specific / usual known errors.
#[cfg(feature = "alloc")]
pub type GenericError = Result<(), Box<dyn BaseError>>;

#[cfg(not(feature = "alloc"))]
pub type GenericError = Result<(), ()>;

/// `VideoError` defines several error types useful when dealing with video / graphics related
/// code.
#[derive(Debug)]
pub enum VideoError {
    /// Generic VESA (usually VBE) related error.
    VesaError,

    #[cfg(feature = "alloc")]
    /// Generic error.
    Exception(Box<dyn BaseError>),

    #[cfg(not(feature = "alloc"))]
    /// Generic error.
    Exception,
}

impl BaseError for VideoError {}

/// `IOError` defines several error types useful when communicating with input/output devices or
/// components.
#[derive(Debug)]
pub enum IOError {
    /// Operation resulted in a timeout.
    IOTimeout,

    /// Invalid I/O command
    InvalidCommand,

    #[cfg(feature = "alloc")]
    /// Generic error.
    Exception(Box<dyn BaseError>),

    #[cfg(not(feature = "alloc"))]
    /// Generic error.
    Exception,

    Unknown,
}

impl BaseError for IOError {}

/// `ClockError` defines several error types useful when dealing with clock related components
/// (such as HPET, TSC or RTC).
#[derive(Debug)]
pub enum ClockError {
    /// The clock is not present / available on the system, or an error was raised when checking
    /// for its availability.
    NotPresent,

    /// Error while trying to calibrate a clock.
    CalibrationError,

    #[cfg(feature = "alloc")]
    /// Generic error.
    Exception(Box<dyn BaseError>),

    #[cfg(not(feature = "alloc"))]
    /// Generic error.
    Exception,
}

#[derive(Debug)]
pub struct E820Error {}
impl E820Error {
    pub fn new() -> Self {
        Self {}
    }
}

impl BaseError for E820Error {}

impl BaseError for ClockError {}

#[cfg(feature = "alloc")]
impl BaseError for TryReserveError {}

#[cfg(feature = "alloc")]
impl BaseError for Utf8Error {}
