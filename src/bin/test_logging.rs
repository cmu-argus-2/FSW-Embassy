#![no_std]
#![no_main]

use defmt::{ info, debug, error };
use embassy_executor::task;
use embassy_rp::{Peri, bind_interrupts, peripherals::USB};
use embassy_time::Timer;
use embassy_time::Instant;
use panic_probe as _;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<embassy_rp::peripherals::USB>;
});

#[task]
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

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());

    spawner.spawn(defmtusb_wrapper(p.USB));

    //delay needed for setting up usb connection
    Timer::after_secs(3).await;

    info!("Starting loop");
    loop {
        info!("Hello, world!  {=u64:tms}\n", Instant::now().as_millis());
        debug!("This is a debug log\n");
        error!("This is an error log\n");
        Timer::after_millis(1000).await;
    }
}