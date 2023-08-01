use core::{arch::asm, ptr};

use core::mem;

const VESA_VBE_BUFFER: u16 = 0xc000;
static mut VESA_VBE_MODE_COUNT: u32 = 0;

#[repr(C, packed)]
pub struct VbeInfoBlock {
    vbe_signature: [u8; 4],
    pub vbe_version: u16,
    oem_string_ptr: u32,
    capabilities: [u8; 4],
    pub video_mode_ptr: u32,
    total_memory: u16,
    reserved: [u8; 492],
}

pub fn real_query_vbeinfo() -> Option<&'static VbeInfoBlock> {
    let pre_sig: *const u8 = "VBE2".as_bytes().as_ptr();

    unsafe {
        ptr::write_volatile(VESA_VBE_BUFFER as *mut [u8; 4], *(pre_sig as *mut [u8; 4]));
    }

    let result: u16;
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

    if result != 0x4f {
        return None;
    }

    let vbe_info: &VbeInfoBlock = unsafe { mem::transmute(VESA_VBE_BUFFER as *mut VbeInfoBlock) };

    Some(vbe_info)
}

pub struct VesaVideoModes {
    counter: u32,
    info_blk: VbeInfoBlock,
}

impl Iterator for VesaVideoModes {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        let mode_ptr = self.info_blk.video_mode_ptr;

        let curr_mode: u16 = unsafe { ptr::read((mode_ptr + 2 * self.counter) as *const u16) };
        if curr_mode == 0xffff {
            return None;
        }

        Some(curr_mode)
    }
}

#[repr(C, packed)]
pub struct ModeInfoBlock {
    mode_attributes: u16,
    window_a_attrs: u8,
    window_b_attrs: u8,
    win_granularity: u16,
    win_size: u16,
    win_a_segment: u16,
    win_b_segment: u16,
    win_func_ptr: u32,
    bytes_per_scanline: u16,
    width: u16,
    height: u16,
    char_width: u8,
    char_height: u8,
    planes_count: u8,
    bits_per_pixel: u8,
    banks_count: u8,
    memory_model: u8,
    bank_size: u8,
    image_pages_count: u8,
    padding_1: u8,
    red_mask_s: u8,
    red_field_pos: u8,
    green_mask_s: u8,
    green_field_pos: u8,
    blue_mask_s: u8,
    blue_field_pos: u8,
    rsvd_mask_size: u8,
    rsvd_field_pos: u8,
    direct_color_mode: u8,
    framebuffer: u32,
    padding_2: u8,
    padding_3: u16,
    reserved: [u8; 206],
}
