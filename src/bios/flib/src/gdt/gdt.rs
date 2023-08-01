use core;
use core::arch::asm;
use core::ptr::write_volatile;

pub struct Gdtr {
    size: u16,
    offset: u32,
    nb_segment: u16,
}

pub struct SegmentDescriptor {
    bytes: [u8; 8],
}

impl SegmentDescriptor {
    pub fn new() -> Self {
        Self { bytes: [0x00; 8] }
    }

    pub fn set_base(&mut self, base: u32) {
        let bytes = u32::to_le_bytes(base);
        self.bytes[2] = bytes[0];
        self.bytes[3] = bytes[1];
        self.bytes[4] = bytes[2];
        self.bytes[7] = bytes[3];
    }

    pub fn set_limit(&mut self, limit: u32) {
        let bytes = u32::to_le_bytes(limit);
        self.bytes[0] = bytes[0];
        self.bytes[1] = bytes[1];
        self.bytes[6] = bytes[2] | self.bytes[6];
    }

    pub fn set_access_byte(&mut self, access_byte: u8) {
        self.bytes[5] = access_byte;
    }

    pub fn set_flags(&mut self, flags: u8) {
        self.bytes[6] = (flags << 4) | self.bytes[6]
    }
}

impl Gdtr {
    pub fn new() -> Self {
        Self {
            size: 0x00,
            nb_segment: 0,
            offset: 0,
        }
    }

    pub fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    pub fn write(&self) {
        unsafe { write_volatile(self.offset as *mut u16, self.size) }
        unsafe { write_volatile((self.offset + 2) as *mut u32, self.offset) }
        unsafe { write_volatile((self.offset + 6) as *mut u16, 0x00) }
    }

    pub fn add_segment(&mut self, segment: SegmentDescriptor) {
        self.nb_segment += 1;
        let size = ((self.nb_segment + 1) * 8 - 1) as u16;
        self.size = size;
        unsafe {
            write_volatile(
                (self.offset + (self.nb_segment as u32) * 8) as *mut SegmentDescriptor,
                segment,
            )
        }
        self.write();
    }

    pub fn load(self) {
        unsafe {
            asm!(
            "lgdt [{0}]",
            in(reg) self.offset
            )
        }
    }
}
