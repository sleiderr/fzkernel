pub struct Writer;

use core::fmt::Write;
impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for ch in s.bytes() {
            crate::video_io::io::__bios_printc(ch);
        }
        Ok(())
    }
}
