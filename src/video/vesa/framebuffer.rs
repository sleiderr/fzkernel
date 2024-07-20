//! Implements a `TextFrameBuffer` that operates on an underlying
//! graphic physical linear framebuffer to display text based objects
//! to the string.
//!
//! It is the default graphic mode when initially entering protected
//! mode.

use core::{fmt::Write, slice};

use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight, RasterizedChar};
use spin::Mutex;
use unifont::{get_glyph, Glyph};

use crate::{
    boot::multiboot::mb_information::FramebufferMultibootInformation,
    mem::{MemoryAddress, VirtAddr},
    video::vesa::video_mode::{ModeInfoBlock, PixelLayout},
};

/// Default font char height.
pub const CHAR_HEIGHT: RasterHeight = RasterHeight::Size16;

/// Default font char width.
pub const CHAR_WIDTH: usize =
    noto_sans_mono_bitmap::get_raster_width(FontWeight::Regular, CHAR_HEIGHT);

/// Default spacing in between characters for the [`TextFrameBuffer`].
pub const CHAR_SPACING: usize = 0;

/// Default space in between text lines for the [`TextFrameBuffer`].
pub const LINE_SPACING: usize = 4;

/// Default padding for the [`TextFrameBuffer`].
pub const BORDER: usize = 6;

/// Default background color for the [`TextFrameBuffer`].
pub const DEFAULT_BG_COLOR: RgbaColor = RgbaColor(26, 28, 34, 0);

/// A text-based buffer.
///
/// It references an underlying physical linear framebuffer,
/// Such buffer can be obtained using the VESA VBE utilities,
/// by requesting a display mode with linear frame buffer
/// enabled. All of the required informations can be found
/// in the corresponding [`ModeInfoBlock`].
///
/// A `TextCursor` makes sure that we can track the current
/// position of the cursor. Line switching , as well as carriage
/// return are implemented by default.
pub struct TextFrameBuffer<'b> {
    pub buffer: &'b mut [u8],
    pub cursor: TextCursor,
    pub metadata: FrameBufferMetadata,
}

/// Locked version of the [`TextFrameBuffer`].
///
/// Uses a [`Mutex`] for synchronization purposes.
///
/// This is the buffer that will be globally defined, under a `static`
/// definition, and initialized when entering protected mode.
pub struct LockedTextFrameBuffer<'b> {
    pub buffer: Mutex<TextFrameBuffer<'b>>,
}

impl<'b> LockedTextFrameBuffer<'b> {
    pub fn new(buff: TextFrameBuffer<'b>) -> Self {
        let buffer = Mutex::new(buff);
        Self { buffer }
    }
}

/// A `TextCursor` tracks the current position of the cursor
/// in a [`TextFrameBuffer`].
pub struct TextCursor {
    pub x: usize,
    pub y: usize,
}

/// Metadata associated with a [`TextFrameBuffer`].
///
/// These informations can often be obtained from a [`ModeInfoBlock`]
/// associated to a VESA display mode, and thus can be obtained using
/// the real mode utils.
pub struct FrameBufferMetadata {
    pub layout: PixelLayout,
    pub bytes_per_px: usize,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bg_color: Option<RgbaColor>,
}

impl Default for TextCursor {
    fn default() -> Self {
        Self {
            x: BORDER,
            y: BORDER,
        }
    }
}

impl<'b> TextFrameBuffer<'b> {
    /// Creates a `TextFrameBuffer` from a VESA display mode and its
    /// associated [`ModeInfoBlock`].
    ///
    /// The VESA mode must support a linear framebuffer.
    pub fn from_vesamode_info(info: &ModeInfoBlock) -> Self {
        let metadata = FrameBufferMetadata {
            layout: info.pixel_layout(),
            bytes_per_px: info.bits_per_pixel as usize >> 3,
            width: info.width as usize,
            height: info.height as usize,
            stride: info.bytes_per_scanline as usize / (info.bits_per_pixel >> 3) as usize,
            bg_color: Some(DEFAULT_BG_COLOR),
        };

        let buffer = unsafe {
            slice::from_raw_parts_mut(
                info.framebuffer as *mut u8,
                (info.bits_per_pixel as usize >> 3) * info.height as usize * info.width as usize,
            )
        };

        let mut framebuffer = Self {
            buffer,
            cursor: TextCursor::default(),
            metadata,
        };

        framebuffer.clear();

        framebuffer
    }

    /// Creates a `TextFrameBuffer` from information provided in a [`MultibootInformation`] block.
    ///
    /// The VESA mode must support a linear framebuffer.
    pub fn from_multiboot_info(
        info: &FramebufferMultibootInformation,
        mapping_addr: VirtAddr,
    ) -> Self {
        let pixel_layout = match (
            info.red_field_pos,
            info.green_field_pos,
            info.blue_field_pos,
        ) {
            (0, 8, 16) => PixelLayout::RGB,
            (16, 8, 0) => PixelLayout::BGR,
            _ => PixelLayout::RGB,
        };
        let metadata = FrameBufferMetadata {
            layout: pixel_layout,
            bytes_per_px: info.bpp as usize >> 3,
            width: info.width as usize,
            height: info.height as usize,
            stride: usize::try_from(info.pitch).expect("invalid framebuffer pitch"),
            bg_color: Some(DEFAULT_BG_COLOR),
        };

        let buffer = unsafe {
            slice::from_raw_parts_mut(
                mapping_addr.as_mut_ptr::<u8>(),
                (info.bpp as usize >> 3) * info.height as usize * info.width as usize,
            )
        };

        let mut framebuffer = Self {
            buffer,
            cursor: TextCursor::default(),
            metadata,
        };

        framebuffer.clear();

        framebuffer
    }

    /// Write a string slice into the [`TextFrameBuffer`].
    pub fn write_str_with_color(&mut self, text: &str, color: &RgbaColor) {
        for c in text.chars() {
            self.putchar(c, Some(color));
        }
    }

    pub fn write_str_bitmap(&mut self, text: &str) {
        for c in text.chars() {
            self.putchar_bitmap(c, false);
        }
    }

    pub fn write_str_bitmap_reversed(&mut self, text: &str) {
        for c in text.chars() {
            self.putchar_bitmap(c, true);
        }
    }

    pub fn write_str_bitmap_centered(&mut self, text: &str, reversed: bool) {
        let text_width = text.len() * 8;
        let remaining_width = self.metadata.width - text_width;

        for _ in 0..remaining_width >> 4 {
            self.putchar_bitmap(' ', false);
        }

        for c in text.chars() {
            self.putchar_bitmap(c, reversed);
        }

        for _ in 0..remaining_width >> 4 {
            self.putchar_bitmap(' ', false);
        }
    }

    /// Prints a character in the `TextFrameBuffer`.
    /// Moves the buffer's cursor current position afterwards,
    /// and jumps to the next line if necessary.
    fn putchar(&mut self, ch: char, color: Option<&RgbaColor>) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            ch => {
                if (self.cursor.x + CHAR_WIDTH) >= self.metadata.width {
                    self.newline();
                }
                if (self.cursor.y + CHAR_HEIGHT.val() + BORDER) >= self.metadata.height {
                    self.clear();
                }
                let rendered = render_char(ch);
                match color {
                    Some(color) => self.write_rasterized_char_with_color(rendered, color),
                    None => self.write_rasterized_char(rendered),
                }
            }
        }
    }

    fn putchar_bitmap(&mut self, ch: char, reversed: bool) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            ch => {
                if (self.cursor.x + CHAR_WIDTH) >= self.metadata.width {
                    self.newline();
                }
                if (self.cursor.y + CHAR_HEIGHT.val() + BORDER) >= self.metadata.height {
                    self.clear();
                }
                if let Glyph::Halfwidth(rendered) = get_glyph(ch).unwrap() {
                    if reversed {
                        self.write_bitmap_char_reversed(rendered);
                    } else {
                        self.write_bitmap_char(rendered);
                    }
                }
            }
        }
    }

    // Pixel per pixel write of a colored char into the buffer after it has
    // been turned into a [`RasterizedChar`].
    fn write_rasterized_char_with_color(&mut self, char: RasterizedChar, color: &RgbaColor) {
        for (y, row) in char.raster().iter().enumerate() {
            for (x, &intensity) in row.iter().enumerate() {
                let rendered_color = RgbaColor(
                    ((color.0 as u16 * intensity as u16) / 255) as u8,
                    ((color.1 as u16 * intensity as u16) / 255) as u8,
                    ((color.2 as u16 * intensity as u16) / 255) as u8,
                    color.3,
                );
                self.write_px_with_color(self.cursor.x + x, self.cursor.y + y, rendered_color);
            }
        }
        self.cursor.x += char.width() + CHAR_SPACING;
    }

    /// Pixel per pixel write to the buffer of a char after it has
    /// been turned into a [`RasterizedChar`].
    fn write_rasterized_char(&mut self, char: RasterizedChar) {
        for (y, row) in char.raster().iter().enumerate() {
            for (x, intensity) in row.iter().enumerate() {
                self.write_px_with_intensity(self.cursor.x + x, self.cursor.y + y, *intensity);
            }
        }
        self.cursor.x += char.width() + CHAR_SPACING;
    }

    fn write_bitmap_char(&mut self, char: &[u8; 16]) {
        for (y, row) in char.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                match *row & 1 << (7 - bit) {
                    0 => self.write_px_with_intensity(self.cursor.x + x, self.cursor.y + y, 0),
                    _ => self.write_px_with_intensity(self.cursor.x + x, self.cursor.y + y, 255),
                }
            }
        }
        self.cursor.x += 8 + CHAR_SPACING;
    }

    fn write_bitmap_char_reversed(&mut self, char: &[u8; 16]) {
        for (y, row) in char.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                match *row & 1 << (7 - bit) {
                    0 => self.write_px_with_intensity(self.cursor.x + x, self.cursor.y + y, 255),
                    _ => self.write_px_with_intensity(self.cursor.x + x, self.cursor.y + y, 0),
                }
            }
        }
        self.cursor.x += 8 + CHAR_SPACING;
    }

    /// Write a pixel to the `TextFrameBuffer` given an intensity.
    fn write_px_with_intensity(&mut self, x: usize, y: usize, intensity: u8) {
        let color = RgbaColor(intensity, intensity, intensity, 0);
        self.write_px_with_color(x, y, color);
    }

    /// Writes a pixel to the `TextFrameBuffer` with a given color.
    ///
    /// The color should be given through a `RgbaColor` struct. The Rgba
    /// convention is used as a default, but if the current display mode
    /// uses another convention (Rgb, Bgra, ...), the bytes of the input color
    /// are switched to match that convention.
    fn write_px_with_color(&mut self, x: usize, y: usize, color: RgbaColor) {
        // Rescale the color if a background color is set, to avoid aliasing.
        let effective_color = match self.metadata.bg_color {
            Some(bg_color) => RgbaColor(
                (((color.0 as u16 * (255 - bg_color.0 as u16)) / 255) + bg_color.0 as u16) as u8,
                (((color.1 as u16 * (255 - bg_color.1 as u16)) / 255) + bg_color.1 as u16) as u8,
                (((color.2 as u16 * (255 - bg_color.2 as u16)) / 255) + bg_color.2 as u16) as u8,
                color.3,
            ),
            None => color,
        };
        let color_slice = match self.metadata.layout {
            PixelLayout::RGB => [
                effective_color.0,
                effective_color.1,
                effective_color.2,
                color.3,
            ],
            PixelLayout::BGR => [
                effective_color.2,
                effective_color.1,
                effective_color.0,
                color.3,
            ],
        };
        let bytes_offset = (x + y * self.metadata.stride) * self.metadata.bytes_per_px;

        self.buffer[bytes_offset..(bytes_offset + self.metadata.bytes_per_px)]
            .copy_from_slice(&color_slice[..self.metadata.bytes_per_px]);
    }

    /// Moves the cursor to the next line.
    /// Automatically inserts a carriage return at the same time.
    fn newline(&mut self) {
        self.cursor.y += CHAR_HEIGHT.val() + LINE_SPACING;
        self.carriage_return();
    }

    /// Moves the cursor to the beginning of the current line.
    fn carriage_return(&mut self) {
        self.cursor.x = BORDER;
    }

    /// Clears the `TextFrameBuffer`.
    ///
    /// Resets the background to the background color if defined, or else
    /// full black.
    pub fn clear(&mut self) {
        self.cursor.x = BORDER;
        self.cursor.y = BORDER;

        // A background color was defined
        if let Some(color) = self.metadata.bg_color {
            let bpp = self.metadata.bytes_per_px;
            let px_slice = match self.metadata.layout {
                PixelLayout::RGB => [color.0, color.1, color.2, color.3],
                PixelLayout::BGR => [color.2, color.1, color.0, color.3],
            };
            match bpp {
                3 => {
                    for chk in self.buffer.rchunks_exact_mut(3) {
                        chk.copy_from_slice(&px_slice[..3]);
                    }
                }
                4 => {
                    for chk in self.buffer.rchunks_exact_mut(4) {
                        chk.copy_from_slice(&px_slice);
                    }
                }
                _ => self.buffer.fill(0),
            }
        } else {
            self.buffer.fill(0);
        }
    }

    pub fn set_background(&mut self, color: Option<RgbaColor>) {
        self.metadata.bg_color = color;
    }
}

impl<'b> Write for TextFrameBuffer<'b> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for ch in s.chars() {
            self.putchar(ch, None);
        }
        Ok(())
    }
}

// Get the [`RasterizedChar`] from a raw `char`.
fn render_char(ch: char) -> RasterizedChar {
    get_raster(ch, FontWeight::Regular, CHAR_HEIGHT).unwrap_or_else(|| render_char('�'))
}

/// `RgbaColor` holds the color data for a pixel. Rgba is used as
/// the default convention for all color usage among the program.
///
/// If needed, a conversion to another convention is performed.
#[derive(Clone, Copy)]
pub struct RgbaColor(pub u8, pub u8, pub u8, pub u8);
