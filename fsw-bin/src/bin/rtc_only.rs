#![no_std]
#![no_main]

use core::fmt::Write;
use defmt_rtt as _; 
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use embassy_rp::peripherals::{I2C1, USB};
use embassy_rp::{bind_interrupts};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use heapless::String;
use static_cell::StaticCell;
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
    
    let driver = embassy_rp::usb::Driver::new(p.USB, Irqs);
    let mut usb_config = embassy_usb::Config::new(0x1234, 0x5678);
    usb_config.manufacturer = Some("Argus");
    usb_config.product = Some("RTC Test");
    usb_config.serial_number = Some("1");

    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    static DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = embassy_usb::Builder::new(
        driver,
        usb_config,
        DEVICE_DESCRIPTOR.init([0; 256]),
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    let mut class = CdcAcmClass::new(&mut builder, state, 64);
    let usb = builder.build();
    let _ = spawner.spawn(usb_task(usb)).unwrap();

    let mut pwr = Output::new(p.PIN_42, Level::High);
    pwr.set_high();
    Timer::after_millis(200).await;

    let i2c_periph = I2c::new_async(p.I2C1, p.PIN_47, p.PIN_46, Irqs, i2c::Config::default());
    let mut rtc = DS3231::new(i2c_periph, 0x68);

    // Get current build time captured during compilation (via build.rs)
    let build_ts: i64 = env!("BUILD_EPOCH").parse().unwrap_or(0);

    // Initial Sync Logic:
    // If RTC has lost power, update to build time.
    if let Ok(true) = rtc.lost_power().await {
        let _ = rtc.set_unix_time(build_ts).await;
    }

    loop {
        if let Ok(dt) = rtc.datetime().await {
            let mut out: String<128> = String::new();
            // Printing readable time and date as requested
            let _ = writeln!(out, "Current Date: {:02}/{:02}/{:04}\r", dt.day, dt.month, dt.year);
            let _ = writeln!(out, "Current Time: {:02}:{:02}:{:02} UTC\r\n", dt.hour, dt.minute, dt.second);
            let _ = class.write_packet(out.as_bytes()).await;
        }
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, embassy_rp::usb::Driver<'static, USB>>) {
    usb.run().await;
}
