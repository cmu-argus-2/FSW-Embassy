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


use fsw_lib::drivers::ds3231::DS3231;

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
    Timer::after_millis(100).await;

    let i2c_periph = I2c::new_async(p.I2C1, p.PIN_47, p.PIN_46, Irqs, i2c::Config::default());
    let mut rtc = DS3231::new(i2c_periph, 0x68);

    // Initial Sync Logic (Referencing build time from build.rs)
    let build_ts: i64 = env!("BUILD_EPOCH").parse().unwrap_or(0);

    if let Ok(true) = rtc.lost_power().await {
        let _ = rtc.set_unix_time(build_ts).await;
        info!("[AUTO-SYNC] Clock calibrated to build time: {}", build_ts);
    } else {
        info!("[SYSTEM] Battery OK - Resuming Internal Time");
    }

    loop {
        match rtc.datetime().await {
            Ok(dt) => {
                info!("Current Date: {:02}/{:02}/{:04}", dt.day, dt.month, dt.year);
                info!("Current Time: {:02}:{:02}:{:02} UTC", dt.hour, dt.minute, dt.second);
            }
            Err(_) => error!("RTC Read Error"),
        }
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn usb_logger_task(usb: Peri<'static, USB>) {
    let driver = embassy_rp::usb::Driver::new(usb, Irqs);
    let config = embassy_usb::Config::new(0x1234, 0x5678);
    defmt_embassy_usbserial::run(driver, config).await;
}
