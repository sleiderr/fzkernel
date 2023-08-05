//! VBE display mode utilities

use core::{arch::asm, ptr};

use core::mem;

use crate::vbe_const;

/// In-memory location of the [`VbeInfoBlock`] header
pub const VESA_VBE_BUFFER: u16 = 0x4f00;

// In-memory location of the [`ModeInfoBlock`] header for the currently
// selected display mode.
pub const VESA_MODE_BUFFER: u16 = 0x4f00 + mem::size_of::<VbeInfoBlock>() as u16;

vbe_const!(VBE_RET_SUPPORTED, 0x4f);
vbe_const!(VBE_RET_SUCCESS, 0x00);
vbe_const!(VBE_SUCCESS, (VBE_RET_SUCCESS << 8) | VBE_RET_SUPPORTED);

/// VBE Controller information block
/// Provides general information about general capabilities
/// of the installed VBE software and hardware.
#[repr(C, packed)]
pub struct VbeInfoBlock {
    // VBE signature
    // Should equal 'VESA' in a valid block
    vbe_signature: [u8; 4],

    // VBE standard version
    // BCD coded value (hi: major, lo: minor)
    pub vbe_version: u16,

    // Real mode pointer to an OEM-defined string
    oem_string_ptr: u32,

    // These bits indicate the support of specific
    // features
    capabilities: u32,

    // Pointer to the list of available video modes.
    // Each mode number occupies one word, the list
    // is terminated by 0xFFFF (-1).
    pub video_mode_ptr: u32,

    // Maximum amount of memory available to the frame
    // buffer in 64KB units.
    total_memory: u16,

    reserved: [u8; 492],
}

/// Initializes the VESA controller and sets the VESA video
/// mode to the desired one.
///
/// The format of the mode number is the following:
///
/// 0                      7                     14  15
/// ----------------------------------------------------
/// |     mode number      |     reserved      | L | C |
/// ----------------------------------------------------
///
/// LI = 1 : use a linear frame buffer
/// CL = 1 : clear display memory
///
/// Can only be used while in real mode, or through a vm86
/// monitor.
#[cfg(feature = "real")]
pub fn real_set_vesa_mode(mode: u16) -> Result<(), ()> {
    use crate::rerror;

    let result: u16;

    // INT 10H
    // VBE 02h call: Set VBE Mode
    //
    // Input:  AX = 0x4f02
    //         BX = Desired mode
    //
    // Output: AX = VBE return status
    unsafe {
        asm!(
        "push ebx",
        "mov ax, 0x4f02",
        "mov bx, cx",
        "int 0x10",
        "pop ebx",
        in("cx") mode,
        out("ax") result
        );
    }

    if result != VBE_SUCCESS {
        rerror!("Failed to set VESA mode");
        return Err(());
    }

    Ok(())
}

/// Returns informations about a specific VESA video mode.
/// It fills a [`ModeInfoBlock`] struct that holds all of
/// these informations.
///
/// The mode number has to be one that was returned by the
/// `real_query_vbeinfo` call.
///
/// Can only be used while in real mode, or through a vm86
/// monitor.
#[cfg(feature = "real")]
pub fn real_query_modeinfo(mode: u16) -> Option<ModeInfoBlock> {
    let mut mode_info: ModeInfoBlock = unsafe { mem::zeroed() };
    let mode_info_ptr: *mut ModeInfoBlock = unsafe { &mut mode_info };
    let result: u16;

    // INT 10H
    // VBE 01h call: Return VBE Mode Information
    //
    // Input:  AX = 0x4f01
    //         CX = Mode number
    //         ES:DI = Pointer to `ModeInfoBlock` structure
    //
    // Output: AX = VBE return status
    unsafe {
        asm!(
            "push es",
            "push di",
            "mov di, ax",
            "xor ax, ax",
            "mov es, ax",
            "mov ax, 0x4f01",
            "int 0x10",
            "pop di",
            "pop es",
            in("cx") mode,
            inout("ax") mode_info_ptr as u16 => result
        );
    }

    if result != VBE_SUCCESS {
        return None;
    }

    Some(mode_info)
}

/// Returns informations about the capabilities of the
/// display controller, and VBE specific informations.
/// It fills a [`VbeInfoBlock`] struct that holds all of
/// these informations.
///
/// That block is kept in memory at address `VESA_VBE_BUFFER`
///
/// Can only be used while in real mode, or through a vm86
/// monitor.
#[cfg(feature = "real")]
pub fn real_query_vbeinfo() -> Option<&'static VbeInfoBlock> {
    // Set the `vbe_signature` field to 'VBE2' in order to
    // query VBE 2.0 informations.
    let pre_sig: *const u8 = "VBE2".as_bytes().as_ptr();

    unsafe {
        ptr::write_volatile(VESA_VBE_BUFFER as *mut [u8; 4], *(pre_sig as *mut [u8; 4]));
    }

    let result: u16;

    // INT 10H
    // VBE 00h call: Return VBE Controller Information
    //
    // Input:  AX = 0x4f00
    //         ES:DI = Pointer to a `VbeInfoBlock`
    //                 structure
    //
    // The `VbeSignature` field should be 'VBE2' when
    // the block is 512 bytes in size.
    //
    // Output: AX = VBE return status
    unsafe {
        asm!(
            "push es",
            "push di",
            "mov di, ax",
            "xor ax, ax",
            "mov es, ax",
            "mov ax, 0x4f00",
            "int 0x10",
            "pop di",
            "pop es",
            inout("ax") VESA_VBE_BUFFER => result
        );
    }

    if result != VBE_SUCCESS {
        return None;
    }

    let vbe_info: &VbeInfoBlock = unsafe { mem::transmute(VESA_VBE_BUFFER as *mut VbeInfoBlock) };

    Some(vbe_info)
}

/// Mode information block that contains technical details
/// relative to a specific display mode.
#[repr(C, align(256))]
pub struct ModeInfoBlock {
    // These bits describe the main characteristics
    // of the display mode.
    pub mode_attributes: u16,

    pub window_a_attrs: u8,
    pub window_b_attrs: u8,
    pub win_granularity: u16,
    pub win_size: u16,
    pub win_a_segment: u16,
    pub win_b_segment: u16,
    pub win_func_ptr: u32,
    pub bytes_per_scanline: u16,

    // Width for this display mode.
    pub width: u16,

    // Height for this display mode.
    pub height: u16,

    pub char_width: u8,
    pub char_height: u8,
    pub planes_count: u8,

    // Number of bits in a pixel.
    pub bits_per_pixel: u8,
    pub banks_count: u8,

    // Specifies the general type of memory organization
    // used for this display mode.
    pub memory_model: MemoryModel,

    pub bank_size: u8,
    pub image_pages_count: u8,
    padding_1: u8,
    pub red_mask_s: u8,
    pub red_field_pos: u8,
    pub green_mask_s: u8,
    pub green_field_pos: u8,
    pub blue_mask_s: u8,
    pub blue_field_pos: u8,
    pub rsvd_mask_size: u8,
    pub rsvd_field_pos: u8,
    pub direct_color_mode: u8,

    /// Physical linear address of the start of the framebuffer
    /// for this mode.
    pub framebuffer: u32,
  
    padding_2: u8,
    padding_3: u16,
    reserved: [u8; 206],
}

impl ModeInfoBlock {
    pub fn pixel_layout(&self) -> PixelLayout {
        match (
            self.red_field_pos,
            self.green_field_pos,
            self.blue_field_pos,
        ) {
            (0, 8, 16) => PixelLayout::RGB,
            (16, 8, 0) => PixelLayout::BGR,
            _ => PixelLayout::RGB,
        }
    }
}

/// Byte order convention for a pixel.
#[derive(Debug, Clone, Copy)]
pub enum PixelLayout {
    RGB,
    BGR,
}

/// Memory organization used type used for a display mode.
#[repr(u8)]
pub enum MemoryModel {
    TextMode = 0,
    CGA = 1,
    Hercules = 2,
    Planar = 3,
    PackedPixel = 4,
    NonChain4 = 5,
    DirectColor = 6,
    YUV = 7,
}

vbe_const!(VBE_MODEATTR_SUPPORTED, 0x1);
vbe_const!(VBE_MODEATTR_TTYSUPPORT, 0x4);
vbe_const!(VBE_MODEATTR_COLOR, 0x8);
vbe_const!(VBE_MODEATTR_GRAPHIC, 0x10);
vbe_const!(VBE_MODEATTR_NOTVGA, 0x20);
vbe_const!(VBE_MODEATTR_LINEAR, 0x80);

/// Wrapper over a [`VbeInfoBlock`] that implements `Iterator`,
/// that can be used to iterate over the available VESA display
/// video modes number.
pub struct VesaVideoModes {
    pub counter: u32,
    pub info_blk: &'static VbeInfoBlock,
}

impl VesaVideoModes {
    pub fn new(info_blk: &'static VbeInfoBlock) -> Self {
        Self {
            counter: 0,
            info_blk,
        }
    }
}

impl Iterator for VesaVideoModes {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        let mode_ptr = self.info_blk.video_mode_ptr;

        let curr_mode: u16 = unsafe { ptr::read((mode_ptr + 2 * self.counter) as *const u16) };

        // Video mode list is terminated by a -1 (0xFFFF)
        if curr_mode == 0xffff {
            return None;
        }

        self.counter += 1;
        Some(curr_mode)
    }
}
