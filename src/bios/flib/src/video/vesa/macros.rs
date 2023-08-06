//! General purpose macros for text output.

use crate::video::vesa::framebuffer::RgbaColor;

/// Base color when displaying context
pub const CTX_COLOR: RgbaColor = RgbaColor(234, 190, 124, 0);

/// Base color when displaying errors
pub const ERR_COLOR: RgbaColor = RgbaColor(239, 35, 60, 0);

/// Prints to the output, and append a new line.
///
/// Writes into the shared [`TextFrameBuffer`].
/// It acquires the lock for the duration of the write.
///
/// # Panics
///
/// Panics if called before initialiazing the shared [`TextFrameBuffer`]
///
/// # Examples
/// ```
/// use flib::println;
///
/// println!("Welcome {} !", "user");
/// ```
#[allow_internal_unstable(format_args_nl)]
#[macro_export]
macro_rules! println {

    () => {
        $crate::video::vesa::print("\n");
    };

    ($($arg: tt)*) => {{
        $crate::video::vesa::arg_print(format_args_nl!($($arg)*))
    }};
}

/// Prints an error message to the output, and append a new line.
///
/// Writes into the shared [`TextFrameBuffer`].
/// It acquires the lock for the duration of the write operation.
///
/// # Panics
///
/// Panics if called before initialiazing the shared [`TextFrameBuffer`]
///
/// # Examples
/// ```
/// use flib::eprintln;
///
/// eprintln!("failed to initialize paging");
/// ```
#[allow_internal_unstable(format_args_nl)]
#[macro_export]
macro_rules! eprintln {
    ($($arg: tt)*) => {
        $crate::video::vesa::print("error: ");
        $crate::video::vesa::arg_print(format_args_nl!($($arg)*))
    };
}

/// Prints a standard information message to the output.
///
/// Writes into the shared [`TextFrameBuffer`].
/// You can specify a 'context' as the first argument when
/// calling the macro, which will be inserted at the beginning
/// of the message.
///
/// # Panics
///
/// Panics if called before initialiazing the shared [`TextFrameBuffer`]
///
/// # Examples
///
/// ```
/// use flib::info;
///
/// info!("paging", "paging enabled");
/// ```
#[allow_internal_unstable(format_args_nl)]
#[macro_export]
macro_rules! info {
    // A context was provided, so we insert it at the beginning of
    // the message.
    ($ctx: literal, $($arg: tt)*) => {
        $crate::video::vesa::print("[info] ");
        $crate::video::vesa::print_colored($ctx, &$crate::video_io::vesa::macros::CTX_COLOR);
        $crate::video::vesa::print(" : ");
        $crate::video::vesa::arg_print(format_args_nl!($($arg)*))
    };
    ($($arg: tt)*) => {
        $crate::video::vesa::print("[info] ");
        $crate::video::vesa::arg_print(format_args_nl!($($arg)*))
    };
}

/// Prints a standard error message to the output.
///
/// Writes into the shared [`TextFrameBuffer`].
/// You can specify a 'context' as the first argument when
/// calling the macro, which will be inserted at the beginning
/// of the error message.
///
/// # Panics
///
/// Panics if called before initialiazing the shared [`TextFrameBuffer`].
///
/// # Examples
///
/// ```
/// use flib::error;
///
/// error!("paging", "failed to initialize paging");
/// ```
#[allow_internal_unstable(format_args_nl)]
#[macro_export]
macro_rules! error {
    // A context was provided, so we insert it at the beginning of
    // the message.
    ($ctx: literal, $($arg: tt)*) => {
        $crate::video::vesa::print_colored("[error] ", &$crate::video_io::vesa::macros::ERR_COLOR);
        $crate::video::vesa::print_colored($ctx, &$crate::video_io::vesa::macros::CTX_COLOR);
        $crate::video::vesa::print(" : ");
        $crate::video::vesa::arg_print(format_args_nl!($($arg)*))
    };
    ($($arg: tt)*) => {
        $crate::video::vesa::print("[error] ");
        $crate::video::vesa::arg_print(format_args_nl!($($arg)*))
    };
}
