//! RTC (Real-Time Clock) Driver
//!
//! The RTC is a hardware clock that keeps track of the current date and time
//! even when the system is powered off. It uses CMOS memory accessed via
//! ports 0x70 (address) and 0x71 (data).
//!
//! # RTC Registers
//! - 0x00: Seconds (0-59)
//! - 0x02: Minutes (0-59)
//! - 0x04: Hours (0-23 or 1-12 depending on format)
//! - 0x06: Weekday (1-7, Sunday = 1)
//! - 0x07: Day of month (1-31)
//! - 0x08: Month (1-12)
//! - 0x09: Year (0-99, last two digits)
//! - 0x32: Century (19-20, if supported)
//! - 0x0A: Status Register A
//! - 0x0B: Status Register B (contains BCD/binary and 12/24h flags)

use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::port::Port;

/// Default timezone offset from UTC (in hours), read from Kconfig
const DEFAULT_TIMEZONE: i8 = crate::config::TIMEZONE;

/// CMOS Address Port - used to select which register to read/write
const CMOS_ADDRESS: u16 = 0x70;
/// CMOS Data Port - used to read/write data from/to the selected register
const CMOS_DATA: u16 = 0x71;

/// RTC Register addresses
mod regs {
    pub const SECONDS: u8 = 0x00;
    pub const MINUTES: u8 = 0x02;
    pub const HOURS: u8 = 0x04;
    pub const WEEKDAY: u8 = 0x06;
    pub const DAY: u8 = 0x07;
    pub const MONTH: u8 = 0x08;
    pub const YEAR: u8 = 0x09;
    pub const CENTURY: u8 = 0x32;
    pub const STATUS_A: u8 = 0x0A;
    pub const STATUS_B: u8 = 0x0B;
}

/// Represents the current date and time with timezone information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub weekday: u8,
    /// Timezone offset from UTC in hours (-12 to +14)
    pub timezone_offset: i8,
}

impl DateTime {
    /// Create a new DateTime struct with the default timezone
    pub fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        weekday: u8,
    ) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            weekday,
            timezone_offset: DEFAULT_TIMEZONE,
        }
    }

    /// Create a new DateTime with a specific timezone offset
    pub fn with_timezone(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        weekday: u8,
        timezone_offset: i8,
    ) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            weekday,
            timezone_offset,
        }
    }

    /// Create a UTC DateTime (timezone offset = 0)
    pub fn new_utc(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        weekday: u8,
    ) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            weekday,
            timezone_offset: 0,
        }
    }

    /// Format the datetime as a string in ISO 8601 format with timezone
    /// Example: "2024-06-15 14:30:45+08:00"
    pub fn to_iso8601(&self) -> alloc::string::String {
        let tz_sign = if self.timezone_offset >= 0 { '+' } else { '-' };
        let tz_hours = self.timezone_offset.abs();
        alloc::format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}{}{:02}:00",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            tz_sign,
            tz_hours
        )
    }

    /// Format as ISO 8601 without timezone info (naive datetime)
    pub fn to_naive_iso8601(&self) -> alloc::string::String {
        alloc::format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second
        )
    }

    /// Convert this datetime to UTC
    pub fn to_utc(&self) -> Self {
        if self.timezone_offset == 0 {
            return *self;
        }
        let total_seconds = self.to_seconds_of_day();
        let utc_seconds = total_seconds as i32 - (self.timezone_offset as i32 * 3600);

        let mut days_to_subtract = 0i32;
        let mut seconds_of_day = utc_seconds;

        // Handle day boundary crossings
        while seconds_of_day < 0 {
            seconds_of_day += 86400;
            days_to_subtract += 1;
        }
        while seconds_of_day >= 86400 {
            seconds_of_day -= 86400;
            days_to_subtract -= 1;
        }

        let (new_year, new_month, new_day, new_weekday) = add_days_to_date(
            self.year,
            self.month,
            self.day,
            self.weekday,
            -days_to_subtract,
        );

        let (new_hour, new_minute, new_second) = seconds_to_hms(seconds_of_day as u32);

        Self::new_utc(
            new_year,
            new_month,
            new_day,
            new_hour,
            new_minute,
            new_second,
            new_weekday,
        )
    }

    /// Convert this UTC datetime to a specific timezone
    pub fn to_timezone(&self, timezone_offset: i8) -> Self {
        if timezone_offset == self.timezone_offset {
            return *self;
        }

        // First convert to UTC if not already
        let utc = if self.timezone_offset == 0 {
            *self
        } else {
            self.to_utc()
        };

        if timezone_offset == 0 {
            return utc;
        }

        // Then convert from UTC to target timezone
        let total_seconds = utc.to_seconds_of_day();
        let local_seconds = total_seconds as i32 + (timezone_offset as i32 * 3600);

        let mut days_to_add = 0i32;
        let mut seconds_of_day = local_seconds;

        // Handle day boundary crossings
        while seconds_of_day < 0 {
            seconds_of_day += 86400;
            days_to_add -= 1;
        }
        while seconds_of_day >= 86400 {
            seconds_of_day -= 86400;
            days_to_add += 1;
        }

        let (new_year, new_month, new_day, new_weekday) =
            add_days_to_date(utc.year, utc.month, utc.day, utc.weekday, days_to_add);

        let (new_hour, new_minute, new_second) = seconds_to_hms(seconds_of_day as u32);

        Self::with_timezone(
            new_year,
            new_month,
            new_day,
            new_hour,
            new_minute,
            new_second,
            new_weekday,
            timezone_offset,
        )
    }

    /// Convert to local time using the configured default timezone
    pub fn to_local(&self) -> Self {
        self.to_timezone(DEFAULT_TIMEZONE)
    }

    /// Get the total seconds since midnight (0 to 86399)
    fn to_seconds_of_day(&self) -> u32 {
        (self.hour as u32) * 3600 + (self.minute as u32) * 60 + (self.second as u32)
    }
}

impl Default for DateTime {
    fn default() -> Self {
        Self {
            year: 2000,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            weekday: 6, // Saturday
            timezone_offset: DEFAULT_TIMEZONE,
        }
    }
}

/// RTC Driver structure
pub struct Rtc {
    address_port: Port<u8>,
    data_port: Port<u8>,
    /// Century register might not be supported on all systems
    has_century_register: bool,
}

impl Rtc {
    /// Create a new RTC instance
    const fn new() -> Self {
        Self {
            address_port: Port::new(CMOS_ADDRESS),
            data_port: Port::new(CMOS_DATA),
            has_century_register: true, // Will be detected at runtime
        }
    }

    /// Read a byte from a CMOS register
    ///
    /// # Safety
    /// This function disables interrupts while accessing the CMOS to prevent
    /// race conditions with other code that might access the same ports.
    unsafe fn read_register(&mut self, reg: u8) -> u8 {
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.address_port.write(reg);
            // Small delay to allow the hardware to respond
            // Reading from an unused port is a common technique
            let _: u8 = x86_64::instructions::port::Port::new(0x80).read();
            self.data_port.read()
        })
    }

    /// Write a byte to a CMOS register
    ///
    /// # Safety
    /// This function disables interrupts while accessing the CMOS.
    unsafe fn write_register(&mut self, reg: u8, value: u8) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.address_port.write(reg);
            let _: u8 = x86_64::instructions::port::Port::new(0x80).read();
            self.data_port.write(value);
        })
    }

    /// Read Status Register B which contains format flags
    unsafe fn read_status_b(&mut self) -> u8 {
        self.read_register(regs::STATUS_B)
    }

    /// Check if RTC is using BCD format (true) or binary format (false)
    unsafe fn is_bcd_format(&mut self) -> bool {
        // Bit 2 of Status B: 0 = BCD, 1 = Binary
        (self.read_status_b() & 0x04) == 0
    }

    /// Check if RTC is using 12-hour format (true) or 24-hour format (false)
    unsafe fn is_12_hour_format(&mut self) -> bool {
        // Bit 1 of Status B: 0 = 12-hour, 1 = 24-hour
        (self.read_status_b() & 0x02) == 0
    }

    /// Convert a BCD value to binary
    const fn bcd_to_binary(bcd: u8) -> u8 {
        ((bcd >> 4) * 10) + (bcd & 0x0F)
    }

    /// Wait for the RTC to finish updating
    ///
    /// The RTC updates once per second. This function waits until the
    /// update is complete to ensure we read a consistent time value.
    unsafe fn wait_for_update(&mut self) {
        // Wait until Update In Progress (UIP) bit is set
        while (self.read_register(regs::STATUS_A) & 0x80) != 0 {
            core::hint::spin_loop();
        }
        // Wait until UIP bit is clear (update complete)
        while (self.read_register(regs::STATUS_A) & 0x80) != 0 {
            core::hint::spin_loop();
        }
    }

    /// Read the current date and time from the RTC (returns UTC time)
    ///
    /// This function handles:
    /// - BCD to binary conversion if needed
    /// - 12-hour to 24-hour conversion if needed
    /// - Century calculation if century register is not available
    ///
    /// Note: RTC hardware stores time in UTC by convention
    pub fn read_datetime(&mut self) -> DateTime {
        unsafe {
            // Wait for any ongoing update to complete
            self.wait_for_update();

            // Read all registers at once
            let mut second = self.read_register(regs::SECONDS);
            let mut minute = self.read_register(regs::MINUTES);
            let mut hour = self.read_register(regs::HOURS);
            let mut day = self.read_register(regs::DAY);
            let mut month = self.read_register(regs::MONTH);
            let mut year = self.read_register(regs::YEAR);
            let weekday = self.read_register(regs::WEEKDAY);

            // Read century register if available
            let century = if self.has_century_register {
                self.read_register(regs::CENTURY)
            } else {
                0
            };

            // Check format flags
            let is_bcd = self.is_bcd_format();
            let is_12_hour = self.is_12_hour_format();

            // Convert from BCD if necessary
            if is_bcd {
                second = Self::bcd_to_binary(second);
                minute = Self::bcd_to_binary(minute);
                hour = Self::bcd_to_binary(hour);
                day = Self::bcd_to_binary(day);
                month = Self::bcd_to_binary(month);
                year = Self::bcd_to_binary(year);
            }

            // Handle 12-hour to 24-hour conversion if necessary
            if is_12_hour {
                // In 12-hour mode, bit 7 indicates PM
                let is_pm = (hour & 0x80) != 0;
                hour = hour & 0x7F; // Clear PM bit

                if hour == 12 {
                    // 12 PM = 12:00, 12 AM = 00:00
                    if !is_pm {
                        hour = 0;
                    }
                } else if is_pm {
                    hour += 12;
                }
            } else {
                // In 24-hour mode, clear bit 7 if set (it's not used)
                hour = hour & 0x7F;
            }

            // Calculate full year
            let full_year = if self.has_century_register && century != 0 {
                let century_bin = if is_bcd {
                    Self::bcd_to_binary(century)
                } else {
                    century
                };
                (century_bin as u16 * 100) + year as u16
            } else {
                // Assume 2000+ for years 0-99
                // This is a reasonable assumption for modern systems
                if year < 80 {
                    2000 + year as u16
                } else {
                    1900 + year as u16
                }
            };

            // Return as UTC (timezone_offset = 0)
            DateTime::new_utc(full_year, month, day, hour, minute, second, weekday)
        }
    }

    /// Set the RTC date and time (expects UTC time)
    ///
    /// # Arguments
    /// * `dt` - The DateTime to set (should be in UTC)
    ///
    /// # Safety
    /// This function modifies the RTC hardware state.
    pub unsafe fn set_datetime(&mut self, dt: &DateTime) {
        // Convert to UTC first if needed
        let utc_dt = if dt.timezone_offset == 0 {
            *dt
        } else {
            dt.to_utc()
        };

        let is_bcd = self.is_bcd_format();
        let is_12_hour = self.is_12_hour_format();

        // Convert values to RTC format
        let (second, minute, hour, day, month, year, century) = if is_bcd {
            (
                ((utc_dt.second / 10) << 4) | (utc_dt.second % 10),
                ((utc_dt.minute / 10) << 4) | (utc_dt.minute % 10),
                if is_12_hour {
                    // Convert 24h to 12h format with PM bit
                    if utc_dt.hour == 0 {
                        0x12 // 12 AM
                    } else if utc_dt.hour < 12 {
                        ((utc_dt.hour / 10) << 4) | (utc_dt.hour % 10)
                    } else if utc_dt.hour == 12 {
                        0x12 | 0x80 // 12 PM
                    } else {
                        let h = utc_dt.hour - 12;
                        (((h / 10) << 4) | (h % 10)) | 0x80
                    }
                } else {
                    ((utc_dt.hour / 10) << 4) | (utc_dt.hour % 10)
                },
                ((utc_dt.day / 10) << 4) | (utc_dt.day % 10),
                ((utc_dt.month / 10) << 4) | (utc_dt.month % 10),
                ((((utc_dt.year % 100) / 10) << 4) | ((utc_dt.year % 100) % 10)) as u8,
                Some(((utc_dt.year / 100) as u8 / 10) << 4 | ((utc_dt.year / 100) as u8 % 10)),
            )
        } else {
            (
                utc_dt.second,
                utc_dt.minute,
                if is_12_hour {
                    if utc_dt.hour == 0 {
                        12 // 12 AM
                    } else if utc_dt.hour < 12 {
                        utc_dt.hour
                    } else if utc_dt.hour == 12 {
                        12 | 0x80 // 12 PM
                    } else {
                        (utc_dt.hour - 12) | 0x80
                    }
                } else {
                    utc_dt.hour
                },
                utc_dt.day,
                utc_dt.month,
                (utc_dt.year % 100) as u8,
                Some((utc_dt.year / 100) as u8),
            )
        };

        // Disable interrupts while setting the time
        x86_64::instructions::interrupts::without_interrupts(|| {
            // Disable NMI (Non-Maskable Interrupt) and select Status Register B
            self.address_port.write(regs::STATUS_B);

            // Write the values
            self.write_register(regs::SECONDS, second);
            self.write_register(regs::MINUTES, minute);
            self.write_register(regs::HOURS, hour);
            self.write_register(regs::WEEKDAY, utc_dt.weekday);
            self.write_register(regs::DAY, day);
            self.write_register(regs::MONTH, month);
            self.write_register(regs::YEAR, year);

            if self.has_century_register {
                if let Some(c) = century {
                    self.write_register(regs::CENTURY, c);
                }
            }
        });
    }

    /// Get the Unix timestamp for the current datetime (UTC)
    ///
    /// Note: This is a simplified calculation and doesn't account for
    /// leap seconds or historical calendar changes.
    pub fn unix_timestamp(&mut self) -> u64 {
        let dt = self.read_datetime();
        datetime_to_unix_timestamp(&dt)
    }
}

lazy_static! {
    /// Global RTC instance protected by a mutex
    pub static ref RTC: Mutex<Rtc> = Mutex::new(Rtc::new());
}

/// Convert a DateTime to Unix timestamp
///
/// This is a simplified calculation that assumes the Gregorian calendar
/// and doesn't account for leap seconds.
pub fn datetime_to_unix_timestamp(dt: &DateTime) -> u64 {
    // Days in each month (non-leap year)
    const DAYS_IN_MONTH: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    // Count days from 1970 to the start of the current year
    let mut days: u64 = 0;

    for year in 1970..dt.year {
        days += if is_leap_year(year) { 366 } else { 365 };
    }

    // Add days for months in current year
    for month in 1..dt.month {
        let month_days = if month == 2 && is_leap_year(dt.year) {
            29
        } else {
            DAYS_IN_MONTH[(month - 1) as usize]
        };
        days += month_days;
    }

    // Add days in current month
    days += (dt.day - 1) as u64;

    // Convert to seconds
    let seconds =
        days * 86400 + (dt.hour as u64) * 3600 + (dt.minute as u64) * 60 + (dt.second as u64);

    seconds
}

/// Convert seconds to hours, minutes, seconds
const fn seconds_to_hms(total_seconds: u32) -> (u8, u8, u8) {
    let hours = (total_seconds / 3600) as u8;
    let remaining = total_seconds % 3600;
    let minutes = (remaining / 60) as u8;
    let seconds = (remaining % 60) as u8;
    (hours, minutes, seconds)
}

/// Add (or subtract) days to a date, handling month/year boundaries
/// Returns (year, month, day, weekday)
fn add_days_to_date(
    year: u16,
    month: u8,
    day: u8,
    weekday: u8,
    days_to_add: i32,
) -> (u16, u8, u8, u8) {
    const DAYS_IN_MONTH: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    let mut new_year = year;
    let mut new_month = month;
    let mut new_day = day as i32;
    let mut new_weekday = weekday as i32;

    // Handle weekday (1-7, wrap around)
    new_weekday = ((new_weekday - 1 + days_to_add) % 7 + 7) % 7 + 1;

    new_day += days_to_add;

    while new_day > 0 {
        let days_in_current_month = if new_month == 2 && is_leap_year(new_year) {
            29
        } else {
            DAYS_IN_MONTH[(new_month - 1) as usize]
        } as i32;

        if new_day <= days_in_current_month {
            break;
        }

        new_day -= days_in_current_month;
        new_month += 1;

        if new_month > 12 {
            new_month = 1;
            new_year += 1;
        }
    }

    while new_day <= 0 {
        new_month -= 1;

        if new_month == 0 {
            new_month = 12;
            new_year -= 1;
        }

        let days_in_prev_month = if new_month == 2 && is_leap_year(new_year) {
            29
        } else {
            DAYS_IN_MONTH[(new_month - 1) as usize]
        } as i32;

        new_day += days_in_prev_month;
    }

    (new_year, new_month, new_day as u8, new_weekday as u8)
}

/// Check if a year is a leap year
const fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Get the current UTC date and time from the RTC
///
/// This is a convenience function that acquires the RTC lock and reads the time.
pub fn now_utc() -> DateTime {
    RTC.lock().read_datetime()
}

/// Get the current local date and time (using configured timezone)
///
/// This is a convenience function that reads UTC time and converts to local time.
pub fn now_local() -> DateTime {
    RTC.lock().read_datetime().to_local()
}

/// Get the current Unix timestamp (UTC)
///
/// This is a convenience function that acquires the RTC lock and returns
/// the Unix timestamp.
pub fn unix_timestamp() -> u64 {
    RTC.lock().unix_timestamp()
}

/// Initialize the RTC subsystem
///
/// This function can be called during system initialization to verify
/// the RTC is functioning properly.
pub fn init() {
    let utc = now_utc();
    let local = utc.to_local();
    log::info!(
        "RTC initialized: UTC={}, Local={} (timezone={:+})",
        utc.to_naive_iso8601(),
        local.to_naive_iso8601(),
        DEFAULT_TIMEZONE
    );
}

/// Get the default timezone offset from Kconfig
pub const fn default_timezone() -> i8 {
    DEFAULT_TIMEZONE
}

/// Convert a UTC timestamp to local time using the default timezone
pub fn utc_to_local_timestamp(utc_timestamp: u64) -> u64 {
    (utc_timestamp as i64 + (DEFAULT_TIMEZONE as i64 * 3600)) as u64
}

/// Convert a local timestamp to UTC using the default timezone
pub fn local_to_utc_timestamp(local_timestamp: u64) -> u64 {
    (local_timestamp as i64 - (DEFAULT_TIMEZONE as i64 * 3600)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_bcd_to_binary() {
        assert_eq!(Rtc::bcd_to_binary(0x00), 0);
        assert_eq!(Rtc::bcd_to_binary(0x01), 1);
        assert_eq!(Rtc::bcd_to_binary(0x10), 10);
        assert_eq!(Rtc::bcd_to_binary(0x99), 99);
        assert_eq!(Rtc::bcd_to_binary(0x59), 59);
        assert_eq!(Rtc::bcd_to_binary(0x23), 23);
    }

    #[test_case]
    fn test_is_leap_year() {
        assert!(is_leap_year(2000));
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2020));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2023));
        assert!(!is_leap_year(2021));
    }

    #[test_case]
    fn test_datetime_to_unix_timestamp() {
        // Unix epoch: 1970-01-01 00:00:00
        let epoch = DateTime::new_utc(1970, 1, 1, 0, 0, 0, 4);
        assert_eq!(datetime_to_unix_timestamp(&epoch), 0);

        // 2000-01-01 00:00:00
        let y2k = DateTime::new_utc(2000, 1, 1, 0, 0, 0, 6);
        assert_eq!(datetime_to_unix_timestamp(&y2k), 946684800);
    }

    #[test_case]
    fn test_datetime_to_iso8601() {
        let dt = DateTime::with_timezone(2024, 6, 15, 14, 30, 45, 6, 8);
        assert_eq!(dt.to_iso8601(), "2024-06-15 14:30:45+08:00");

        let dt_utc = DateTime::new_utc(2024, 6, 15, 14, 30, 45, 6);
        assert_eq!(dt_utc.to_iso8601(), "2024-06-15 14:30:45+00:00");

        let dt_negative = DateTime::with_timezone(2024, 6, 15, 14, 30, 45, 6, -5);
        assert_eq!(dt_negative.to_iso8601(), "2024-06-15 14:30:45-05:00");
    }

    #[test_case]
    fn test_to_utc() {
        // Test +8 timezone to UTC conversion
        let local = DateTime::with_timezone(2024, 6, 15, 14, 30, 0, 6, 8);
        let utc = local.to_utc();
        assert_eq!(utc.hour, 6); // 14 - 8 = 6
        assert_eq!(utc.timezone_offset, 0);

        // Test crossing midnight
        let local = DateTime::with_timezone(2024, 6, 15, 2, 30, 0, 6, 8);
        let utc = local.to_utc();
        assert_eq!(utc.hour, 18); // 2 - 8 = -6 -> 18 (previous day)
        assert_eq!(utc.day, 14);

        // Test -5 timezone to UTC conversion
        let local = DateTime::with_timezone(2024, 6, 15, 14, 30, 0, 6, -5);
        let utc = local.to_utc();
        assert_eq!(utc.hour, 19); // 14 + 5 = 19
    }

    #[test_case]
    fn test_to_timezone() {
        let utc = DateTime::new_utc(2024, 6, 15, 14, 0, 0, 6);

        // Convert to +8
        let local = utc.to_timezone(8);
        assert_eq!(local.hour, 22); // 14 + 8 = 22

        // Convert to -5
        let eastern = utc.to_timezone(-5);
        assert_eq!(eastern.hour, 9); // 14 - 5 = 9
    }

    #[test_case]
    fn test_add_days_to_date() {
        // Add days
        let (y, m, d, w) = add_days_to_date(2024, 6, 15, 6, 5);
        assert_eq!((y, m, d), (2024, 6, 20));
        assert_eq!(w, 4); // Thursday

        // Subtract days
        let (y, m, d, w) = add_days_to_date(2024, 6, 15, 6, -10);
        assert_eq!((y, m, d), (2024, 6, 5));

        // Cross month boundary
        let (y, m, d, _) = add_days_to_date(2024, 6, 15, 6, 20);
        assert_eq!((y, m, d), (2024, 7, 5));

        // Cross year boundary
        let (y, m, d, _) = add_days_to_date(2024, 1, 5, 1, -10);
        assert_eq!((y, m, d), (2023, 12, 26));
    }
}
