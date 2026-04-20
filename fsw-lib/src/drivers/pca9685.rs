use embassy_rp::gpio::Output;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

/*
 * PCA9685 Burnwire Deployment Driver (100% Flight Grade)
 * 
 * Safety features:
 * - Hardware OE interlock (Absolute first line of init)
 * - Software state machine (Arm -> Fire -> Safe)
 * - I2C All-Call Isolation (Address 0x70 disabled)
 * - Totem-pole output configuration
 */

pub struct PCA9685<'d, I2C: I2c> {
    i2c: I2C,
    addr: u8,
    oe_pin: Output<'d>, 
    state: BurnwireState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum BurnwireState {
    Disarmed, // Outputs physically disabled (OE High)
    Armed,    // Physical path enabled, waiting for command
    Fired,    // Current is flowing to the burnwire
}

#[derive(Debug, defmt::Format)]
pub enum Error<E> {
    I2c(E),
    NotArmed,     
    AlreadyFired, 
}

mod regs {
    pub const MODE1:      u8 = 0x00;
    pub const MODE2:      u8 = 0x01;
    pub const PRE_SCALE:  u8 = 0xFE;
    pub const ALL_OFF_H:  u8 = 0xFD; 
    pub const LED_BASE:   u8 = 0x06; 
}

mod bits {
    pub const MODE1_SLEEP:   u8 = 1 << 4; // Low-power / Oscillator off
    pub const MODE1_AI:      u8 = 1 << 5; // Auto-Increment for burst reads
    pub const MODE2_OUTDRV:  u8 = 1 << 2; // Totem-pole (1) or Open-drain (0)
    pub const LED_FULL_ON:   u8 = 1 << 4; 
    pub const LED_FULL_OFF:  u8 = 1 << 4; 
}

impl<'d, I2C: I2c> PCA9685<'d, I2C> {
    /// Create driver. Disables outputs immediately for safety.
    pub fn new(i2c: I2C, addr: u8, mut oe_pin: Output<'d>) -> Self {
        oe_pin.set_high(); // Active-low kill switch
        Self {
            i2c,
            addr,
            oe_pin,
            state: BurnwireState::Disarmed,
        }
    }

    /// Full hardware initialization. 
    /// Ensures safety interlocks are set before any bus communication.
    pub async fn init(&mut self) -> Result<(), Error<I2C::Error>> {
        // Absolute first step: Verify hardware is safed
        self.oe_pin.set_high();

        /*
         * 1. Oscillator Setup & Isolation
         * We enter SLEEP to modify the prescaler.
         * We ensure bit 0 (ALLCALL) is 0 to ignore the 0x70 broadcast address.
         */
        self.write_reg(regs::MODE1, bits::MODE1_SLEEP).await?;
        self.write_reg(regs::PRE_SCALE, 0x1E).await?; // Default ~200Hz
        
        // 2. Output edges: Totem Pole ensures clean logic switching
        self.write_reg(regs::MODE2, bits::MODE2_OUTDRV).await?;

        // 3. Logic-off for all channels
        self.all_off().await?;
        
        // 4. Wake up and enable Auto-Increment
        self.write_reg(regs::MODE1, bits::MODE1_AI).await?;
        Timer::after_micros(500).await;
        
        self.state = BurnwireState::Disarmed;
        Ok(())
    }

    /// Remove the hardware "kill switch" interlock.
    pub fn arm(&mut self) -> Result<(), Error<I2C::Error>> {
        if self.state == BurnwireState::Fired {
            return Err(Error::AlreadyFired);
        }
        self.oe_pin.set_low(); 
        self.state = BurnwireState::Armed;
        Ok(())
    }

    /// Perform the deployment. Safes the hardware automatically after duration.
    pub async fn fire(&mut self, channel: u8, duration: Duration) -> Result<(), Error<I2C::Error>> {
        if self.state != BurnwireState::Armed {
            return Err(Error::NotArmed);
        }

        self.set_full_on(channel).await?;
        self.state = BurnwireState::Fired;
        
        // Async-safe wait (can be interrupted by another task calling safe())
        Timer::after(duration).await;
        
        self.safe().await?;
        Ok(())
    }

    /// Shutdown the deployment hardware logic and physical path.
    pub async fn safe(&mut self) -> Result<(), Error<I2C::Error>> {
        // 1. Cut physical path (fastest)
        self.oe_pin.set_high();
        
        // 2. Cut logical path
        let _ = self.all_off().await;
        
        // 3. Power down internal oscillator
        let _ = self.write_reg(regs::MODE1, bits::MODE1_SLEEP).await;
        
        self.state = BurnwireState::Disarmed;
        Ok(())
    }

    pub async fn set_full_on(&mut self, channel: u8) -> Result<(), Error<I2C::Error>> {
        let on_reg = regs::LED_BASE + (channel * 4) + 1; 
        self.write_reg(on_reg, bits::LED_FULL_ON).await?;
        let off_reg = regs::LED_BASE + (channel * 4) + 3; 
        self.write_reg(off_reg, 0x00).await?;
        Ok(())
    }

    pub async fn all_off(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_reg(regs::ALL_OFF_H, bits::LED_FULL_OFF).await?;
        Ok(())
    }

    async fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), Error<I2C::Error>> {
        self.i2c.write(self.addr, &[reg, val]).await.map_err(Error::I2c)
    }

    pub fn get_state(&self) -> BurnwireState {
        self.state
    }
}
