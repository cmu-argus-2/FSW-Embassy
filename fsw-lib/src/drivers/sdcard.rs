use embedded_sdmmc::{TimeSource, Timestamp};

/*
 * SD Card TimeSource Implementation
 * 
 * The SD card filesystem needs a way to timestamp files when they are created
 * or modified. Since we don't always have a synced RTC at boot, we use 
 * this simple timesource.
 */
pub struct SdTimeSource;

impl TimeSource for SdTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        // Default to a fixed date (2024-01-01 00:00:00) until the system 
        // provides a real-time update.
        Timestamp {
            year_since_1970: 54, // 1970 + 54 = 2024
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}
