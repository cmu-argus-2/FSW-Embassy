use embedded_hal_async::i2c::{I2c as I2cAsync, ErrorType};

type I2cResult<T, I2cError> = core::result::Result<T, <I2cError as ErrorType>::Error>;

const DATA_V_MASK: u8 = 0xF0;
const DATA_I_MASK: u8 = 0x0F;

// Status register
const STATUS_READ: u8 = 0x1 << 6;
// _STATUS_ADC_OC = const(0x1 << 0)
const STATUS_ADC_ALERT: u8 = 0x1 << 1;
// _STATUS_HS_OC = const(0x1 << 2)
// STATUS_HS_ALERT = const(0x1 << 3)
const STATUS_OFF_STATUS: u8 = 0x1 << 4;
// STATUS_OFF_ALERT = const(0x1 << 5)

// Extended registers
const ALERT_EN_EXT_REG_ADDR: u8 = 0x81;
const ALERT_EN_EN_ADC_OC4: u8 = 0x1 << 1;
const ALERT_EN_CLEAR: u8 = 0x1 << 4;

const ALERT_TH_EN_REG_ADDR: u8 = 0x82;

const CONTROL_REG_ADDR: u8 = 0x83;
const CONTROL_SWOFF: u8 = 0x1 << 0;


pub struct ADM1176<I2C: I2cAsync> {
    i2c: I2C,
    addr: u8,
    sense_resistor: f32,
    // on: bool,
    overcurrent_level: u8,
    v_fs_over_res: f32,
    i_fs_over_res: f32,
}

impl<I2C: I2cAsync> ADM1176<I2C> {
    pub fn new(i2c: I2C, addr: u8) -> Self {
        Self {
            i2c: i2c,
            addr: addr,
            sense_resistor: 0.01,
            // on: true,
            overcurrent_level: 0xFF,
            v_fs_over_res: 26.35 / 4096.0,
            i_fs_over_res: 0.10584 / 4096.0,
        }
    }

    pub async fn config(&mut self, values: &[&str]) -> I2cResult<(), I2C> {
        const V_CONT_BIT: u8 = 0x1 << 0;
        const V_ONCE_BIT: u8 = 0x1 << 1;
        const I_CONT_BIT: u8 = 0x1 << 2;
        const I_ONCE_BIT: u8 = 0x1 << 3;
        const V_RANGE_BIT: u8 = 0x1 << 4;
        
        let mut config:u8 = 0;
        for value in values.iter() {
            match *value {
                "V_CONT" => config |= V_CONT_BIT,
                "V_ONCE" => config |= V_ONCE_BIT,
                "I_CONT" => config |= I_CONT_BIT,
                "I_ONCE" => config |= I_ONCE_BIT,
                "V_RANGE" => config |= V_RANGE_BIT,
                _ => {}
            }
        }
        self.i2c.write(self.addr, &[config]).await 
    }

    pub async fn read_voltage_current(&mut self) -> I2cResult<(f32, f32), I2C> {
        let mut buf = [0u8; 3];
        match self.i2c.read(self.addr, &mut buf).await {
            Ok(_) => {
                let raw_voltage = (((buf[0] as u16) << 8) | ((buf[2] & 0xF0) as u16)) >> 4;
                let raw_current = ((buf[1] << 4) | (buf[2] & 0x0F)) as u16;
                let voltage = (self.v_fs_over_res) * raw_voltage as f32;  // volts
                let current = ((self.i_fs_over_res) * raw_current as f32) / self.sense_resistor;  // amperes
                Ok((voltage, current))
            },
            Err(e) => Err(e)
        }
    }

    async fn turn_off(&mut self) -> I2cResult<(), I2C> {
        let mut off: [u8;2] = [CONTROL_REG_ADDR, 0x04 | CONTROL_SWOFF];
        self.i2c.write(self.addr, &mut off).await
    }

    async fn turn_on(&mut self) -> I2cResult<(), I2C> {
        let mut on: [u8;2] = [CONTROL_REG_ADDR, 0x04 & !CONTROL_SWOFF];
        self.i2c.write(self.addr, &mut on).await;
        self.config(&["V_CONT", "I_CONT"]).await
    }

    pub async fn set_device_on(&mut self, on: bool) -> I2cResult<(), I2C> {
        if on {
            self.turn_on().await
        } else {
            self.turn_off().await
        }
    }

    pub async fn device_on(&mut self) -> I2cResult<bool, I2C> {
        match self.status().await {
            Ok(status) => Ok((status & STATUS_OFF_STATUS) != STATUS_OFF_STATUS),
            Err(e) => Err(e)
        }
    }

    pub fn overcurrent_level(&self) -> u8 {
        self.overcurrent_level
    }

    pub async fn set_overcurrent_level(&mut self, value: u8) -> I2cResult<(), I2C> {
        let mut cmd: [u8;2] = [ALERT_EN_EXT_REG_ADDR, 0x04 | ALERT_EN_EN_ADC_OC4];
        self.i2c.write(self.addr, &mut cmd).await?;
        cmd = [ALERT_TH_EN_REG_ADDR, value];
        let res = self.i2c.write(self.addr, &mut cmd).await?;
        self.overcurrent_level = value;
        Ok(res)
    }

    pub async fn clear(&mut self) -> I2cResult<(), I2C> {
        let mut cmd: [u8;2] = [ALERT_EN_EXT_REG_ADDR, 0x04 | ALERT_EN_CLEAR];
        self.i2c.write(self.addr, &mut cmd).await
    }

    pub async fn status(&mut self) -> I2cResult<u8, I2C> {
        self.i2c.write(self.addr, &[STATUS_READ]).await?;
        let mut status_buf = [0u8; 1];
        self.i2c.read(self.addr, &mut status_buf).await?;
        self.i2c.write(self.addr, &[0x00 & !STATUS_READ]).await?;
        Ok(status_buf[0])
    }
}

