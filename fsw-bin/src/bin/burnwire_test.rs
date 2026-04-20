#![no_std]
#![no_main]

use core::fmt::Write;
use defmt_rtt as _; 
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use embassy_rp::peripherals::{I2C1, USB};
use embassy_rp::{bind_interrupts};
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use heapless::String;
use static_cell::StaticCell;
use {panic_probe as _};


use fsw_lib::drivers::pca9685::PCA9685;

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
    usb_config.product = Some("Burnwire Test");
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
    let _ = spawner.spawn(usb_task(usb));

    // Peripheral Power
    let mut pwr = Output::new(p.PIN_42, Level::High);
    pwr.set_high();
    Timer::after_millis(200).await;

    // FIXME(config): Verify if PIN_10 is the correct Output Enable (OE) for your PCA9685
    let oe_pin = Output::new(p.PIN_10, Level::High);

    let i2c_periph = I2c::new_async(p.I2C1, p.PIN_47, p.PIN_46, Irqs, i2c::Config::default());
    let mut pwm = PCA9685::new(i2c_periph, 0x40, oe_pin);

    // Wait for USB host
    class.wait_connection().await;

    let _ = class.write_packet("PCA9685 Burnwire Driver Init...\r\n".as_bytes()).await;
    match pwm.init().await {
        Ok(_) => { let _ = class.write_packet("PCA9685 Ready\r\n".as_bytes()).await; }
        Err(e) => { 
            let mut s: String<64> = String::new();
            let _ = writeln!(s, "PCA9685 Init Error: {:?}\r\n", e);
            let _ = class.write_packet(s.as_bytes()).await;
        }
    }

    loop {
        let _ = class.write_packet("Arming Burnwire...\r\n".as_bytes()).await;
        match pwm.arm() {
            Ok(_) => {
                let _ = class.write_packet("Armed. Firing Channel 0 for 5s...\r\n".as_bytes()).await;
                match pwm.fire(0, Duration::from_secs(5)).await {
                    Ok(_) => { let _ = class.write_packet("Fire Successful. Safed.\r\n".as_bytes()).await; }
                    Err(e) => {
                        let mut s: String<64> = String::new();
                        let _ = writeln!(s, "Fire Error: {:?}\r\n", e);
                        let _ = class.write_packet(s.as_bytes()).await;
                    }
                }
            }
            Err(e) => {
                let mut s: String<64> = String::new();
                let _ = writeln!(s, "Arm Error: {:?}\r\n", e);
                let _ = class.write_packet(s.as_bytes()).await;
            }
        }
        
        Timer::after_secs(15).await;
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, embassy_rp::usb::Driver<'static, USB>>) {
    usb.run().await;
}
