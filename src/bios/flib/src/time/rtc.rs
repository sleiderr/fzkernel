//! RTC "Real-Time Clock" control utilities.
//!
//! Provides a way to read the current UTC time from the RTC chip.

use crate::{
    time::{DateTime, Weekday},
    io::io_delay
};
use crate::io::{inb, outb};

/// Reads a registry from the CMOS chip.
///
/// Disables interrupts during the operation, and waits a small
/// delay between write and read.
#[inline]
fn __cmos_read(registry: u8) -> u8 {
    // Set the registry to which we read from.
    outb(0x70, registry);
    io_delay();

    // Read the value of the registry.
    inb(0x71)
}

/// Checks if the RTC value is being updated (rolls over).
///
/// Bit 7 of Status register A indicates if an update is ongoing
/// (if set).
#[inline]
fn __rtc_update() -> bool {
    // Status register A: 0x0A
    let stat_reg_a = __cmos_read(0xa);

    // Bit 7 of the status register indicates the update status.
    if stat_reg_a & (1 << 7) != 0 {
        return true;
    }

    false
}

/// Reads the current value of all RTC registers (except alarm and status
/// registers), to retrieve the current UTC time.
#[inline]
fn __cmos_rtc_read() -> [u8; 7] {
    let seconds = __cmos_read(0x00);
    let minutes = __cmos_read(0x02);
    let hours = __cmos_read(0x04);
    let week_day = __cmos_read(0x06);
    let month_day = __cmos_read(0x07);
    let month = __cmos_read(0x08);
    let year = __cmos_read(0x09);

    [seconds, minutes, hours, week_day, month_day, month, year]
}

/// Reads the RTC registers to retrieve the current UTC time, and
/// returns it a [`DateTime`].
///
/// This should only return consistent value, as it avoids reading during
/// RTC updates.
pub fn rtc_read() -> DateTime {
    // Check that no update is ongoing.
    while __rtc_update() {}

    let mut last_time = [0u8; 7];
    let mut time = __cmos_rtc_read();

    // Read the time twice until the values match.
    while last_time != time {
        last_time = time;
        while __rtc_update() {}
        time = __cmos_rtc_read();
    }

    // Translate BCD values to binary.
    // If bit 2 of status register B is clear, the value is in
    // BCD format (eg: if the current seconds count is 53, the value
    // read from the register will be 0x53).
    if (__cmos_read(0xb) & 0x4) == 0 {
        time[0] = (time[0] >> 4) * 10 + (time[0] & 0x0f);
        time[1] = (time[1] >> 4) * 10 + (time[1] & 0x0f);
        time[2] = (time[2] >> 4) * 10 + (time[2] & 0x0f);
        time[4] = (time[4] >> 4) * 10 + (time[4] & 0x0f);
        time[5] = (time[5] >> 4) * 10 + (time[5] & 0x0f);
        time[6] = (time[6] >> 4) * 10 + (time[6] & 0x0f);
    }

    // Translate 12h format to 24h format.
    // If bit 1 of status register B is clear, the value is in 12h format.
    // If the bit 0x80 of the hours register is set, the current time is PM.
    // We need to remove the mask, and convert to 24h time.
    if (__cmos_read(0xb) & 0x2) == 0 && (time[2] & 0x80) != 0 {
        // Midnight is 0 and not 24
        time[2] = ((time[2] & 0x7f) + 12) % 24;
    }

    let weekday = match time[3] {
        1 => Weekday::Sunday,
        2 => Weekday::Monday,
        3 => Weekday::Tuesday,
        4 => Weekday::Wednesday,
        5 => Weekday::Thursday,
        6 => Weekday::Friday,
        7 => Weekday::Saturday,
        _ => Weekday::Monday,
    };

    DateTime {
        seconds: time[0],
        minutes: time[1],
        hours: time[2],
        weekday,
        month_day: time[4],
        month: time[5],
        year: 2000 + time[6] as u16,
    }
}
