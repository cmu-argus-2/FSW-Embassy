#![no_std]
#![no_main]

use defmt::info;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use embassy_rp::peripherals::{I2C1, USB};
use embassy_rp::{Peri, bind_interrupts};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use static_cell::StaticCell;
use {panic_probe as _};


use fsw_lib::drivers::ds3231::DS3231;
use fsw_lib::drivers::opt4003::OPT4003;

type Bus = Mutex<NoopRawMutex, I2c<'static, I2C1, i2c::Async>>;
static BUS: StaticCell<Bus> = StaticCell::new();

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<I2C1>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let _ = spawner.spawn(logger_task(p.USB)).unwrap();
    Timer::after_secs(2).await;

    let mut pwr = Output::new(p.PIN_42, Level::High);
    pwr.set_high();

    let i2c = I2c::new_async(p.I2C1, p.PIN_47, p.PIN_46, Irqs, i2c::Config::default());
    let bus = BUS.init(Mutex::new(i2c));

    let _ = spawner.spawn(rtc_task(bus)).unwrap();
    let _ = spawner.spawn(lux_task(bus)).unwrap();
}

#[embassy_executor::task]
async fn logger_task(usb: Peri<'static, USB>) {
    let driver = embassy_rp::usb::Driver::new(usb, Irqs);
    let config = embassy_usb::Config::new(0x1234, 0x5678);
    defmt_embassy_usbserial::run(driver, config).await;
}

#[embassy_executor::task]
async fn rtc_task(bus: &'static Bus) {
    let mut rtc = DS3231::new(I2cDevice::new(bus), 0x68);
    loop {
        if let Ok(dt) = rtc.datetime().await {
            info!("{}/{}/{} {}:{}:{}", dt.day, dt.month, dt.year, dt.hour, dt.minute, dt.second);
        }
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn lux_task(bus: &'static Bus) {
    let mut lux = OPT4003::new(I2cDevice::new(bus), 0x44);
    let _ = lux.init().await;
    loop {
        if let Ok(val) = lux.lux().await {
            info!("Lux: {}", val);
        }
        Timer::after_secs(2).await;
    }
}
