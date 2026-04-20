#![no_std]
#![no_main]

use core::fmt::Write;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use embassy_rp::peripherals::{I2C0, USB};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embedded_hal_async::i2c::I2c as _;
use heapless::String;
use static_cell::StaticCell;
use {panic_probe as _};

bind_interrupts!(struct Irqs {
    I2C0_IRQ    => InterruptHandler<I2C0>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::crystal(12_000_000);
    let p = embassy_rp::init(config);

    let driver = embassy_rp::usb::Driver::new(p.USB, Irqs);
    let usb_config = embassy_usb::Config::new(0x1234, 0x5678);

    static STATE:             StaticCell<State>      = StaticCell::new();
    static DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR:    StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF:       StaticCell<[u8; 64]>  = StaticCell::new();

    let state = STATE.init(State::new());
    let mut builder = embassy_usb::Builder::new(
        driver, usb_config,
        DEVICE_DESCRIPTOR.init([0; 256]),
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );
    let mut class = CdcAcmClass::new(&mut builder, state, 64);
    let usb = builder.build();
    let _ = spawner.spawn(usb_task(usb)).unwrap();

    // Power on peripherals
    let _pwr = Output::new(p.PIN_42, Level::High);
    Timer::after_millis(500).await;

    // Wait for USB connection
    class.wait_connection().await;

    let mut i2c = I2c::new_async(p.I2C0, p.PIN_5, p.PIN_4, Irqs, i2c::Config::default());

    loop {
        let mut found = false;
        let mut out: String<64> = String::new();
        let _ = writeln!(out, "\r\n--- I2C scan ---\r");
        let _ = class.write_packet(out.as_bytes()).await;

        for addr in 0x08_u8..=0x77 {
            let mut buf = [0u8; 1];
            match i2c.read(addr, &mut buf).await {
                Ok(_) => {
                    let mut out: String<64> = String::new();
                    let _ = writeln!(out, "  found 0x{:02X}\r", addr);
                    let _ = class.write_packet(out.as_bytes()).await;
                    defmt::info!("found 0x{:02X}", addr);
                    found = true;
                }
                Err(_) => {}
            }
        }

        if !found {
            let mut out: String<64> = String::new();
            let _ = writeln!(out, "  nothing found\r");
            let _ = class.write_packet(out.as_bytes()).await;
        }

        Timer::after_secs(5).await;
    }
}

#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<'static, embassy_rp::usb::Driver<'static, USB>>,
) {
    usb.run().await;
}
