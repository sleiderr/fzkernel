#![allow(clippy::module_name_repetitions)]
//! Time related utilities.
//!
//! Implements [`DateTime`] which represents a UTC time, as well as several
//! formats to turn that date into a string (ISO8601, short/long time/date).
//!
//! Uses the RTC on the CMOS chip to retrieve the current UTC time.

pub mod rtc;

use core::fmt::{self, Display};

#[cfg(feature = "alloc")]
use alloc::{format, string::String};
use bytemuck::{Pod, Zeroable};

use crate::x86::tsc::TSC_CLK;

/// Returns the current UTC time as a [`DateTime`], that
/// can then be further formatted.
///
/// This returns a consistent value (avoids RTC rolls over).
///
/// # Examples:
///
/// ```
/// use fzboot::time;
///
/// let curr_time = time::now();
/// println!("{curr_time}");
/// ```
#[must_use]
pub fn date() -> DateTime {
    rtc::rtc_read()
}

/// Returns the current time in microseconds of the TSC counter.
///
/// Can be used for time measurement.
///
/// # Panics
///
/// Panics if called before initializing the `TSC counter`.
///
/// # Examples
///
/// ```
/// use fzboot::time;
///
/// let time_a = time::now();
/// some_function();
/// let time_b = time::now();
///
/// println!("some_func exec time = {:?}", time_b - time_a);
/// ```
#[must_use]
pub fn now() -> f64 {
    TSC_CLK.get().unwrap().tsc_time()
}

/// Waits for a given amount of milliseconds.
///
/// # Examples
///
/// ```
/// use fzboot::time::wait;
///
/// println!("hi !");
/// wait!(1_000);
/// println!("hi from 1 second in the future!");
/// ```
#[macro_export]
macro_rules! wait {
    ($time: literal) => {
        let init_time = $crate::time::now();
        core::hint::spin_loop();
        while $crate::time::now() < (init_time + 1_000_f64 * $time) {}
    };
}

/// Waits until a condition is satisfied, or a timeout is reached.
///
/// # Examples
///
/// Useful to wait until a statement becomes true for only a certain amount of time.
///
/// ```
/// use fzboot::time::wait_for;
///
/// wait_for!(statement, 50);
/// ```
#[macro_export]
macro_rules! wait_for {
    ($cond: expr, $timeout: literal) => {
        let init_time = $crate::time::now();
        core::hint::spin_loop();
        while $crate::time::now() < (init_time + 1_000_f64 * $timeout as f64) && !$cond {}
    };
}

/// Waits until a condition is satisfied, or a timeout is reached.
///
/// If a timeout was reached, an special expression can be executed.
///
/// # Examples
///
/// ```
/// use fzboot::time::wait_for_or;
///
/// wait_for_or!(statement, 50, println!("Timeout was reached !"));
/// ```
#[macro_export]
macro_rules! wait_for_or {
    ($cond: expr, $timeout: literal, $else: expr) => {
        let init_time = $crate::time::now();
        let mut cond_checked: bool = false;
        core::hint::spin_loop();
        while $crate::time::now() < (init_time + 1_000_f64 * $timeout as f64) {
            if $cond {
                cond_checked = true;
                break;
            }
        }

        if !cond_checked {
            $else
        }
    };
}

/// While loop with a timeout.
///
/// # Examples
///
/// Can be used to avoid infinite loops.
///
/// ```
/// use fzboot::time::while_timeout;
///
/// while_timeout!(true, 50, ());
/// ```
#[macro_export]
macro_rules! while_timeout {
    ($cond: expr, $timeout: literal, $body: expr) => {
        let init_time = $crate::time::now();
        while $cond || $crate::time::now() < (init_time + 1_000_f64 * $timeout as f64) {
            $body
        }
    };
}

/// A week day
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// A `DateTime` represents a UTC date and time.
///
/// It supports several display formats:
///
/// - ISO8601 : 2023‐08‐07T09:43:18Z
/// - Longtime: 09:43:18
/// - Shorttime: 09:43
/// - Longdate: Monday, August 7, 2023
/// - Shortdate: 08/07/2023
///
/// # Examples:
///
/// ```
/// use fzboot::time;
///
/// let curr_time: DateTime = time::now();
/// println!("{}", curr_time.format_shorttime()); // 09:43 format
/// println!("{curr_time}"); // ISO8601 full format
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub struct DateTime {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub weekday: Weekday,
    pub month_day: u8,
    pub month: u8,
    pub year: u16,
}

#[cfg(feature = "alloc")]
impl DateTime {
    /// Unix `epoch`
    pub const LINUX_EPOCH: Self = Self {
        seconds: 0,
        minutes: 0,
        hours: 0,
        weekday: Weekday::Thursday,
        month_day: 1,
        month: 1,
        year: 1970,
    };

    /// Converts the `DateTime` to a `String` representation in the
    /// ISO8601 full format (eg: 2023‐08‐07T09:43:18Z). This is the
    /// default representation used by `DateTime`.
    ///
    /// # Examples:
    ///
    /// ```
    /// use fzboot::time;
    ///
    /// let curr_time: DateTime = time::now();
    /// // both in ISO8601 full format
    /// assert_eq!(format!("{}", curr_time), curr_time.format_datetime_iso8601());
    /// ```
    #[must_use]
    pub fn format_datetime_iso8601(&self) -> String {
        format!(
            "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.month_day, self.hours, self.minutes, self.seconds
        )
    }

    /// Converts the `DateTime` to a `String` representation in the ISO8601 format
    /// of the time part (eg: T09:43:18).
    ///
    /// # Examples:
    ///
    /// ```
    /// use fzboot::time;
    ///
    /// let curr_time: DateTime = time::now();
    /// println!("{}", curr_time.format_time_iso8601()); // T09:43:18 format
    /// ```
    #[must_use]
    pub fn format_time_iso8601(&self) -> String {
        format!("T{:02}:{:02}:{:02}", self.hours, self.minutes, self.seconds)
    }

    /// Converts the `DateTime` to a `String` representation in the ISO8601 format
    /// of the date part (eg: 2023-08-07).
    ///
    /// # Examples:
    ///
    /// ```
    /// use fzboot::time;
    ///
    /// let curr_time: DateTime = time::now();
    /// println!("{}", curr_time.format_date_iso8601()); // 2023-08-07 format
    /// ```
    #[must_use]
    pub fn format_date_iso8601(&self) -> String {
        format!("{}-{:02}-{:02}", self.year, self.month, self.month_day)
    }

    /// Converts the `DateTime` to a `String` representation of the date part
    /// in a long human-readable format (eg: Monday, August 7, 2023).
    ///
    /// # Examples:
    ///
    /// ```
    /// use fzboot::time;
    ///
    /// let curr_time: DateTime = time::now();
    /// println!("{}", curr_time.format_longdate()); // Monday, August 7, 2023 format
    /// ```
    #[must_use]
    pub fn format_longdate(&self) -> String {
        let week_day = match self.weekday {
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
            Weekday::Sunday => "Sunday",
        };

        let month = match self.month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "",
        };

        format!("{}, {} {}, {}", week_day, month, self.month_day, self.year)
    }

    /// Converts the `DateTime` to a `String` representation of the date part
    /// in a short human-readable format (eg: 08/07/2023).
    ///
    /// # Examples:
    ///
    /// ```
    /// use fzboot::time;
    ///
    /// let curr_time: DateTime = time::now();
    /// println!("{}", curr_time.format_shortdate()); // 08/07/2023 format
    /// ```
    #[must_use]
    pub fn format_shortdate(&self) -> String {
        format!("{:02}/{:02}/{}", self.month, self.month_day, self.year)
    }

    /// Converts the `DateTime` to a [`String`] representation of the time part
    /// in a long human-readable format (eg: 09:43:18).
    ///
    /// # Examples:
    ///
    /// ```
    /// use fzboot::time;
    ///
    /// let curr_time: DateTime = time::now();
    /// println!("{}", curr_time.format_longtime()); // 09:43:18 format
    /// ```
    #[must_use]
    pub fn format_longtime(&self) -> String {
        format!("{:02}:{:02}:{02}", self.hours, self.minutes, self.seconds)
    }

    /// Converts the `DateTime` to a [`String`] representation of the time part
    /// in a short human-readable format (eg: 09:43).
    ///
    /// # Examples:
    ///
    /// ```
    /// use fzboot::time;
    ///
    /// let curr_time: DateTime = time::now();
    /// println!("{}", curr_time.format_shorttime()); // 09:43 format
    /// ```
    #[must_use]
    pub fn format_shorttime(&self) -> String {
        format!("{:02}:{:02}", self.hours, self.minutes)
    }
}

#[cfg(feature = "alloc")]
impl fmt::Debug for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_datetime_iso8601())
    }
}
#[cfg(feature = "alloc")]
impl Display for DateTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.format_datetime_iso8601())
    }
}

pub(super) const SECS_PER_DAY: i64 = 86_400;
pub(super) const SECS_PER_HOUR: i64 = 3_600;
pub(super) const SECS_PER_MINUTE: i64 = 60;
pub(super) const DAYS_PER_MONTH: [u16; 48] = [
    31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
];
pub(super) const FOUR_YEARS_LEN_IN_SECS: i64 = 126_230_400;

/// _Unix_ timestamps, as used in [`Inode`] metadata.
///
/// # Structure
///
/// The lower 32-bits of the timestamp encode a 32-bit signed integer that represents the number of
/// seconds since the `epoch` ([`DateTime::LINUX_EPOCH`]).
///
/// The high 32-bits encode additional data, to provide nanoseconds precision and two additional
/// bits to widen the maximum seconds count that can be contained in a `UnixTimestamp`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct UnixTimestamp(u64);

impl From<u64> for UnixTimestamp {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl UnixTimestamp {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    /// Returns the numbers of seconds that have elapsed since the _Unix_ `epoch`.
    pub fn raw_seconds(&self) -> i64 {
        let t_val_s: i64 = i64::from(
            TryInto::<u32>::try_into(self.0 & ((1 << 32) - 1)).expect("invalid conversion"),
        );

        let adjustement_bits =
            i64::from(TryInto::<u8>::try_into((self.0 >> 32) & 0b11).expect("invalid conversion"));
        t_val_s + (1 << 32) * adjustement_bits
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    /// Returns the number of nanoseconds contained in this timestamp.
    ///
    /// Available for timestamps with nanosecond precision, returns 0 otherwise.
    pub fn raw_ns(&self) -> u32 {
        ((self.0 >> 34) & ((1 << 30) - 1))
            .try_into()
            .expect("invalid conversion")
    }
}

impl UnixTimestamp {
    /// Unix `epoch`
    pub(super) const INITIAL_TIMESTAMP: Self = Self(0);
}

impl core::ops::Sub for UnixTimestamp {
    type Output = Self;

    fn sub(self, rhs: UnixTimestamp) -> Self::Output {
        UnixTimestamp(self.0.saturating_sub(rhs.0))
    }
}

impl core::ops::Sub<u64> for UnixTimestamp {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self::Output {
        UnixTimestamp(self.0.saturating_sub(rhs))
    }
}

impl core::ops::Div<u64> for UnixTimestamp {
    type Output = u64;

    fn div(self, rhs: u64) -> Self::Output {
        self.0.div_euclid(rhs)
    }
}

impl core::ops::Rem<u64> for UnixTimestamp {
    type Output = u64;

    fn rem(self, rhs: u64) -> Self::Output {
        self.0.rem_euclid(rhs)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct UnixTimestamp32(u32);

fn get_current_date_in_block(days_passed: i64) -> (u16, u16, u64) {
    let mut curr_month = 0;
    let mut day_counter = days_passed;

    while day_counter > 0 {
        if day_counter <= i64::from(DAYS_PER_MONTH[curr_month]) {
            break;
        }
        day_counter -= i64::from(DAYS_PER_MONTH[curr_month]);
        curr_month += 1;
    }

    (
        (curr_month / 12).try_into().expect("invalid conversion"),
        ((curr_month % 12) + 1)
            .try_into()
            .expect("invalid conversion"),
        (day_counter + 1).try_into().expect("invalid conversion"),
    )
}

#[cfg(feature = "alloc")]
impl From<UnixTimestamp> for DateTime {
    fn from(value: UnixTimestamp) -> Self {
        let full_year_blk_count = value.raw_seconds() / FOUR_YEARS_LEN_IN_SECS;
        let remaining_secs = value.raw_seconds() % FOUR_YEARS_LEN_IN_SECS;

        let days_passed = remaining_secs.div_euclid(SECS_PER_DAY);

        let (year_offset, month, day) = get_current_date_in_block(days_passed);

        let years = i64::from(DateTime::LINUX_EPOCH.year)
            + i64::from(year_offset)
            + 4 * full_year_blk_count;

        let seconds = value.raw_seconds() % SECS_PER_DAY;
        let hours = seconds.div_euclid(SECS_PER_HOUR);
        let minutes = seconds
            .rem_euclid(SECS_PER_HOUR)
            .div_euclid(SECS_PER_MINUTE);
        let remaining_secs = seconds
            .rem_euclid(SECS_PER_HOUR)
            .rem_euclid(SECS_PER_MINUTE);

        Self {
            seconds: remaining_secs.try_into().expect("invalid seconds value"),
            minutes: minutes.try_into().expect("invalid minutes value"),
            hours: hours.try_into().expect("invalid hours value"),
            weekday: Weekday::Monday,
            month_day: day.try_into().expect("invalid day value"),
            month: month.try_into().expect("invalid month value"),
            year: years.try_into().expect("invalid year value"),
        }
    }
}
