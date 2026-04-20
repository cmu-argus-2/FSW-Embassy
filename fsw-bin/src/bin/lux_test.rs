#![no_std]
#![no_main]

use defmt::{info, error};
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use embassy_rp::peripherals::{I2C1, USB};
use embassy_rp::{Peri, bind_interrupts};
use embassy_time::Timer;
use {panic_probe as _};


use fsw_lib::drivers::opt4003::OPT4003;

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<I2C1>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::crystal(12_000_000);
    let p = embassy_rp::init(config);
    
    let _ = spawner.spawn(usb_logger_task(p.USB)).unwrap();
    Timer::after_secs(2).await;

    // Power gate
    let mut pwr = Output::new(p.PIN_42, Level::High);
    pwr.set_high();
    Timer::after_millis(200).await;

    // Sun Sensors - testing on I2C1 (SCL=47, SDA=46) as it is known working
    let i2c_periph = I2c::new_async(p.I2C1, p.PIN_47, p.PIN_46, Irqs, i2c::Config::default());
    let mut sensor = OPT4003::new(i2c_periph, 0x44);

    match sensor.init().await {
        Ok(_) => info!("OPT4003 Active at 0x44"),
        Err(_) => error!("OPT4003 Init Failed"),
    }

    loop {
        match sensor.lux().await {
            Ok(lux) => info!("Light Intensity: {} Lux", lux),
            Err(e) => error!("Sensor Error: {:?}", e),
        }
        Timer::after_secs(2).await;
    }
}

#[embassy_executor::task]
async fn usb_logger_task(usb: Peri<'static, USB>) {
    let driver = embassy_rp::usb::Driver::new(usb, Irqs);
    let config = embassy_usb::Config::new(0x1234, 0x5678);
    defmt_embassy_usbserial::run(driver, config).await;
}
