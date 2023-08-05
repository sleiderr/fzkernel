//! VESA related utilities.
//!
//! This modules provides several utilities to set the
//! VESA video mode, and a basic graphic API.
//! The former can only be used while in real mode,
//! or through a vm86 monitor, while the latter is
//! designed for protected mode.

use conquer_once::spin::OnceCell;

use crate::video_io::vesa::framebuffer::LockedTextFrameBuffer;

#[macro_use]
pub mod video_mode;
pub mod framebuffer;

pub static TEXT_BUFFER: OnceCell<LockedTextFrameBuffer> = OnceCell::uninit();

/// Changes the VESA video mode to the closest one given
/// conditions (for now, only width and height). Only
/// keeps video mode that are based on a linear framebuffer.
///
/// This can only run in a real mode execution context, or
/// using a vm86 monitor.
///
/// # Usage
///
/// ```
/// use flib::video_mode::vesa_mode_setup;
///
/// vesa_mode_setup(1920, 1080);
/// ```
///
/// Note: the [`VbeInfoBlock`] is initialized and stored
/// at `VESA_VBE_BUFFER` address.

#[cfg(feature = "real")]
pub fn vesa_mode_setup(x: u16, y: u16) {
    use core::{cmp::Ordering, mem};
    use crate::video_io::vesa::video_mode::*;

    let mut best_mode: u16 = 1;
    let mut best_diff: u32 = u32::max_value();
    let mut best_bpp: u8 = 0;

    let vbe_info_blk = video_mode::real_query_vbeinfo().unwrap();
    let modes = video_mode::VesaVideoModes::new(vbe_info_blk);

    // Iterate over all available modes returned
    for mode in modes {
        if let Some(mode_info) = video_mode::real_query_modeinfo(mode) {
            // We need to make sure the mode uses a linear framebuffer.
            // Bit 7 of the `mode_attributes` equals 1 if a linear
            // framebuffer is available.
            // We also make sure that the mode is a graphic mode (bit
            // 4 of the `mode_attributes`)
            if mode_info.mode_attributes & (VBE_MODEATTR_LINEAR | VBE_MODEATTR_GRAPHIC)
                != (VBE_MODEATTR_LINEAR | VBE_MODEATTR_GRAPHIC)
            {
                continue;
            }

            // We only support packed pixel memory model or direct color,
            // so we skip any display mode that does not use one of these.
            match mode_info.memory_model {
                MemoryModel::PackedPixel | MemoryModel::DirectColor => {}
                _ => {
                    continue;
                }
            }

            // We compute the distance between 2 modes by comparing their euclidean
            // distance to the ideal mode.
            let px_diff = (mode_info.width as u32 - x as u32).pow(2)
                + (mode_info.height as u32 - y as u32).pow(2);

            match px_diff.cmp(&best_diff) {
                // We found a better fit
                Ordering::Less => {
                    best_mode = mode;
                    best_diff = px_diff;
                    best_bpp = mode_info.bits_per_pixel;
                }
                // In case of equality, we (for now) choose the mode with the
                // highest bits per pixel count.
                Ordering::Equal => {
                    if mode_info.bits_per_pixel > best_bpp {
                        best_mode = mode;
                        best_diff = px_diff;
                        best_bpp = mode_info.bits_per_pixel;
                    }
                }
                Ordering::Greater => {}
            }
        }
    }

    // Enable the linear framebuffer (bit 14 of the mode)
    best_mode = best_mode | 0x4000;

    // Set the current mode to the best fit we found
    video_mode::real_set_vesa_mode(best_mode);

    // Keeps the `ModeInfoBlock` of the mode we choose in memory, next to
    // the `VbeInfoBlock`
    let best_info = video_mode::real_query_modeinfo(best_mode).unwrap();
    let info_buffer_ptr =
        (VESA_VBE_BUFFER as usize + mem::size_of::<VbeInfoBlock>()) as *mut ModeInfoBlock;
    unsafe {
        *info_buffer_ptr = best_info;
    }
}

#[macro_export]
macro_rules! vbe_const {
    ($name: tt, $value: expr) => {
        pub const $name: u16 = $value;
    };
}
