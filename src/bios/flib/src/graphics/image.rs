use crate::data::r#abstract::{DataSegment, DataSource, Disk};
use crate::disk_io::disk::AddressPacket;
use crate::video_io::io::{switch_graphic, write};
use core::cmp::max;
use core::mem::size_of;
use core::ptr::read_volatile;
use numtoa::NumToA;

#[repr(C, packed)]
pub struct Image {
    dim: (u16, u16),
}

pub struct Point {
    x: u16,
    y: u16,
}

#[repr(C, packed)]
struct Pixel {
    x: u16,
    y: u16,
    color: u8,
}

impl Point {
    pub fn x(&self) -> u16 {
        self.x
    }

    pub fn y(&self) -> u16 {
        self.y
    }
}

impl Image {
    pub fn draw<S: DataSource>(&self, center: Point, source: S, origin: u32) -> Result<(), ()> {
        switch_graphic();
        let max = max(self.x_size(), self.y_size());
        let n_pixel = self.dim.0 as u64 * self.dim.1 as u64;
        let mut image_segment: DataSegment<[Pixel; 512], S> = DataSegment::default(source);
        image_segment.set_length(2560);
        image_segment.set_physical_origin(origin);
        let mut address = 0x4;
        for pixel in 0..n_pixel / 512 {
            if let Some(p) = image_segment.read_abstract(address) {
                for pixel in p {
                    write(
                        pixel.x + center.x() - self.x_size() * (640 / max) / 2,
                        pixel.y + center.y() - self.y_size() * (480 / max) / 2,
                        pixel.color,
                    );
                }
            } else {
                return Err(());
            }
            address += size_of::<[Pixel; 512]>() as u32;
        }
        Ok(())
    }

    pub fn x_size(&self) -> u16 {
        self.dim.0
    }

    pub fn y_size(&self) -> u16 {
        self.dim.1
    }

    pub fn new(dim: (u16, u16)) -> Self {
        Self { dim }
    }
}

impl Point {
    pub fn new(x: u16, y: u16) -> Self {
        return Self { x, y };
    }
}

impl From for Image {
    fn from_address(address: u32) -> Self {
        let mut address = address;
        let x = unsafe { read_volatile(address as *const u16) };
        address += 2;
        let y = unsafe { read_volatile(address as *const u16) };

        Self { dim: (x, y) }
    }

    fn from_disk(block: u64, buffer: u32) -> Result<Self, ()> {
        let address = AddressPacket::new(1, buffer, block);
        match address.disk_read(0x80) {
            Ok(_) => Ok(Self::from_address(buffer)),
            Err(_) => Err(()),
        }
    }
}

pub trait From {
    fn from_address(address: u32) -> Self;
    fn from_disk(block: u64, buffer: u32) -> Result<Self, ()>
    where
        Self: Sized;
}
