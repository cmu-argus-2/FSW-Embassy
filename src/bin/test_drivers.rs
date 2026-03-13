//! This example shows how to share (async) I2C and SPI buses between multiple devices.

#![no_std]
#![no_main]

use defmt::{info, error};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use embassy_rp::peripherals::{I2C1, USB};

use embassy_rp::{Peri, bind_interrupts, dma};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use static_cell::StaticCell;
use {panic_probe as _};

use rtt_target::rtt_init_print;

//use package name given in Cargo.toml
extern crate embassy_rp2350_examples as rp;
use rp::drivers::adm1176::ADM1176 as adm1176;

type I2c1Bus = Mutex<NoopRawMutex, I2c<'static, I2C1, i2c::Async>>;

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<I2C1>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<embassy_rp::peripherals::USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Here we go!");

    //set up global logger
    rtt_init_print!();

    spawner.spawn(defmtusb_wrapper(p.USB));

    //delay needed to set up usb connection
    Timer::after_secs(3).await;

    //turn on peri
    let mut led = Output::new(p.PIN_42, Level::Low);
    led.set_high();

    // Shared I2C bus
    let i2c = I2c::new_async(p.I2C1, p.PIN_47, p.PIN_46, Irqs, i2c::Config::default());
    static I2C_BUS: StaticCell<I2c1Bus> = StaticCell::new();
    let i2c_bus = I2C_BUS.init(Mutex::new(i2c));

    //spawn adm1176 driver task
    spawner.spawn(i2c_task_a(i2c_bus));


    loop {
        info!("looping...");
        Timer::after_secs(5).await;
    }
}

#[embassy_executor::task]
async fn defmtusb_wrapper(usb: Peri<'static, USB>) {
    let driver = embassy_rp::usb::Driver::new(usb, Irqs);
    let config = {
        let mut c = embassy_usb::Config::new(0x1234, 0x5678);
        c.serial_number = Some("defmt");
        c.max_packet_size_0 = 64;
        c.composite_with_iads = true;
        c.device_class = 0xEF;
        c.device_sub_class = 0x02;
        c.device_protocol = 0x01;
        c
    };
    defmt_embassy_usbserial::run(driver, config).await;
}

#[embassy_executor::task]
async fn i2c_task_a(i2c_bus: &'static I2c1Bus) {
    let i2c_dev = I2cDevice::new(i2c_bus);
    let mut sensor = adm1176::new(i2c_dev, 0x40);
    sensor.config(&["V_CONT", "I_CONT"]).await;
    loop {
        match sensor.read_voltage_current().await {
            Ok((voltage, current)) => {
                info!("voltage {}, current {}", voltage, current);
            }
            Err(e) => {
                error!("{}", e);
            }
        }
        
        Timer::after_secs(1).await;
    }
}