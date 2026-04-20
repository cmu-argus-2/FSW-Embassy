use embedded_hal_async::i2c::I2c;
use chrono::{Datelike, NaiveDateTime, Timelike};

pub struct DS3231<I2C: I2c> {
    i2c: I2C,
    addr: u8,
}

mod regs {
    pub const DATETIME_START: u8 = 0x00;
    pub const CONTROL: u8 = 0x0E;
    pub const STATUS: u8 = 0x0F;
}

mod masks {
    pub const SECONDS: u8 = 0x7F;
    pub const MINUTES: u8 = 0x7F;
    pub const HOURS_24H: u8 = 0x3F;
    pub const DAY_OF_MONTH: u8 = 0x3F;
    pub const MONTH: u8 = 0x1F;
    pub const OSF: u8 = 0x80; // Oscillator Stop Flag
}

const YEAR_OFFSET: u16 = 2000;

#[derive(Debug, defmt::Format)]
pub enum Error<E> {
    I2c(E),
    InvalidTime,
}

#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl<I2C: I2c> DS3231<I2C> {
    pub fn new(i2c: I2C, addr: u8) -> Self {
        Self { i2c, addr }
    }

    fn bcd2dec(bcd: u8) -> u8 { ((bcd & 0xF0) >> 4) * 10 + (bcd & 0x0F) }
    fn dec2bcd(dec: u8) -> u8 { ((dec / 10) << 4) | (dec % 10) }

    pub async fn set_unix_time(&mut self, ts: i64) -> Result<(), Error<I2C::Error>> {
        #[allow(deprecated)]
        let dt = NaiveDateTime::from_timestamp_opt(ts, 0).ok_or(Error::InvalidTime)?;
        self.set_datetime(&DateTime {
            year: dt.year() as u16,
            month: dt.month() as u8,
            day: dt.day() as u8,
            hour: dt.hour() as u8,
            minute: dt.minute() as u8,
            second: dt.second() as u8,
        }).await
    }

    pub async fn set_datetime(&mut self, dt: &DateTime) -> Result<(), Error<I2C::Error>> {
        let buf = [
            regs::DATETIME_START,
            Self::dec2bcd(dt.second) & masks::SECONDS,
            Self::dec2bcd(dt.minute) & masks::MINUTES,
            Self::dec2bcd(dt.hour) & masks::HOURS_24H,
            0x01, // Day of week (1-7), setting to 1 as it is unused
            Self::dec2bcd(dt.day) & masks::DAY_OF_MONTH,
            Self::dec2bcd(dt.month) & masks::MONTH,
            Self::dec2bcd((dt.year % 100) as u8),
        ];
        self.i2c.write(self.addr, &buf).await.map_err(Error::I2c)?;
        
        // Control: Clear /EOSC to 0 (Enable Oscillator)
        self.i2c.write(self.addr, &[regs::CONTROL, 0x00]).await.map_err(Error::I2c)?;
        // Status: Clear OSF to 0 (Oscillator Stop Flag)
        self.i2c.write(self.addr, &[regs::STATUS, 0x00]).await.map_err(Error::I2c)?;
        Ok(())
    }

    pub async fn lost_power(&mut self) -> Result<bool, Error<I2C::Error>> {
        let mut b = [0u8; 1];
        self.i2c.write_read(self.addr, &[regs::STATUS], &mut b).await.map_err(Error::I2c)?;
        Ok((b[0] & masks::OSF) != 0)
    }

    pub async fn datetime(&mut self) -> Result<DateTime, Error<I2C::Error>> {
        let mut b = [0u8; 7];
        self.i2c.write_read(self.addr, &[regs::DATETIME_START], &mut b).await.map_err(Error::I2c)?;
        Ok(DateTime {
            second: Self::bcd2dec(b[0] & masks::SECONDS),
            minute: Self::bcd2dec(b[1] & masks::MINUTES),
            hour:   Self::bcd2dec(b[2] & masks::HOURS_24H),
            day:    Self::bcd2dec(b[4] & masks::DAY_OF_MONTH), // b[3] is Day of Week
            month:  Self::bcd2dec(b[5] & masks::MONTH),
            year:   YEAR_OFFSET + Self::bcd2dec(b[6]) as u16,
        })
    }
}
