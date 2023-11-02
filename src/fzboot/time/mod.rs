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
pub fn date() -> DateTime {
    rtc::rtc_read()
}

/// Returns the current time in microseconds of the TSC counter.
///
/// Can be used for time measurement.
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
        while $crate::time::now() < (init_time + 1_000_f64 * $time as f64) {}
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
#[derive(Debug)]
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
