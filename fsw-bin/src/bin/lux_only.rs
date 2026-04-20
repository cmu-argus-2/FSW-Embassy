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
use heapless::String;
use static_cell::StaticCell;
use {panic_probe as _};


use fsw_lib::drivers::opt4003::OPT4003;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
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
    let _ = spawner.spawn(usb_task(usb));

    // Pattern 8: Drive pin high immediately
    let _pwr = Output::new(p.PIN_42, Level::High);
    Timer::after_millis(500).await;

    // Pattern: Wait for USB host
    class.wait_connection().await;

    // I2C0 on GP5 (SCL) and GP4 (SDA)
    let i2c = I2c::new_async(p.I2C0, p.PIN_5, p.PIN_4, Irqs, i2c::Config::default());
    let mut sensor = OPT4003::new(i2c, 0x44);
    
    // Pattern 3: Address scanning loop
    'scan: loop {
        for &addr in &[0x44u8, 0x45, 0x46, 0x47] {
            sensor.set_addr(addr);
            if sensor.init().await.is_ok() {
                let mut out: String<64> = String::new();
                let _ = writeln!(out, "OPT4003 found at 0x{:02X}\r", addr);
                let _ = class.write_packet(out.as_bytes()).await;
                defmt::info!("OPT4003 found at 0x{:02X}", addr);
                break 'scan;
            }
        }
        let mut out: String<64> = String::new();
        let _ = writeln!(out, "Searching for OPT4003...\r");
        let _ = class.write_packet(out.as_bytes()).await;
        Timer::after_millis(500).await;
    }

    loop {
        match sensor.lux().await {
            Ok(lux) => {
                // Pattern 6: Split float for defmt and serial
                let whole = lux as u32;
                let frac  = ((lux - whole as f32) * 100.0) as u32;
                
                let mut out: String<64> = String::new();
                let _ = writeln!(out, "Lux: {}.{:02}\r", whole, frac);
                let _ = class.write_packet(out.as_bytes()).await;
                defmt::info!("{}.{:02} lux", whole, frac);
            }
            Err(e) => {
                // Pattern 4: Error recovery and re-init
                defmt::warn!("lux failed: {:?}, reinitialising", e);
                if sensor.init().await.is_err() {
                    defmt::error!("re-init failed, resetting system");
                    cortex_m::peripheral::SCB::sys_reset();
                }
            }
        }
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, embassy_rp::usb::Driver<'static, USB>>) {
    usb.run().await;
}
