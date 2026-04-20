#![no_std]
#![no_main]

use core::fmt::Write;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi::{self, Spi};
use embassy_rp::peripherals::USB;
use embassy_rp::bind_interrupts;
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use heapless::String;
use static_cell::StaticCell;
use {panic_probe as _};

use embedded_sdmmc::{SdCard, VolumeManager, VolumeIdx, Mode};
use fsw_lib::drivers::sdcard::SdTimeSource;
use embedded_hal_bus::spi::ExclusiveDevice;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // USB Logging Setup
    let driver = embassy_rp::usb::Driver::new(p.USB, Irqs);
    let mut usb_config = embassy_usb::Config::new(0x1234, 0x5678);
    usb_config.manufacturer = Some("Argus");
    usb_config.product = Some("SD Card Test");

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

    class.wait_connection().await;
    let _ = class.write_packet("\r\n--- SD Card Test ---\r\n".as_bytes()).await;

    let mut _pwr = Output::new(p.PIN_42, Level::High);
    Timer::after_millis(200).await;

    // SPI1 for SD Card
    let mut spi_config = spi::Config::default();
    spi_config.frequency = 400_000; // Start at 400kHz
    let mut spi_bus = Spi::new_blocking(p.SPI1, p.PIN_10, p.PIN_11, p.PIN_12, spi_config);
    let mut cs = Output::new(p.PIN_13, Level::High);

    // 1. Initial Identity Phase
    let init_success = {
        let spi_device = ExclusiveDevice::new(&mut spi_bus, &mut cs, embassy_time::Delay);
        let sdcard = SdCard::new(spi_device, embassy_time::Delay);
        match sdcard.num_bytes() {
            Ok(size) => {
                let mut s: String<64> = String::new();
                let _ = writeln!(s, "Card detected. Size: {} MB\r\n", size / 1024 / 1024);
                let _ = class.write_packet(s.as_bytes()).await;
                true
            }
            Err(e) => {
                let mut s: String<128> = String::new();
                let _ = writeln!(s, "SD Card Init Error: {:?}\r\n", e);
                let _ = class.write_packet(s.as_bytes()).await;
                false
            }
        }
    };

    if !init_success { loop { Timer::after_secs(1).await; } }

    // 2. High-Speed Phase (12MHz)
    spi_bus.set_frequency(12_000_000);
    let _ = class.write_packet("SPI speed boosted to 12MHz.\r\n".as_bytes()).await;

    // Re-initialize for filesystem operations using high speed
    let spi_device = ExclusiveDevice::new(&mut spi_bus, &mut cs, embassy_time::Delay);
    let sdcard = SdCard::new(spi_device, embassy_time::Delay);
    let volume_mgr = VolumeManager::new(sdcard, SdTimeSource);
    
    match volume_mgr.open_volume(VolumeIdx(0)) {
        Ok(volume) => {
            let _ = class.write_packet("Volume 0 (FAT) opened.\r\n".as_bytes()).await;
            match volume.open_root_dir() {
                Ok(root_dir) => {
                    match root_dir.open_file_in_dir("TEST.TXT", Mode::ReadWriteCreateOrAppend) {
                        Ok(mut file) => {
                            let msg = "Argus Satellite: SD Writing Perfected.\n";
                            let _ = file.write(msg.as_bytes());
                            let _ = file.flush();
                            let _ = class.write_packet("Success: Data written and flushed.\r\n".as_bytes()).await;
                        }
                        Err(e) => {
                            let mut s: String<64> = String::new();
                            let _ = writeln!(s, "File Error: {:?}\r\n", e);
                            let _ = class.write_packet(s.as_bytes()).await;
                        }
                    }
                }
                Err(e) => {
                    let mut s: String<64> = String::new();
                    let _ = writeln!(s, "Dir Error: {:?}\r\n", e);
                    let _ = class.write_packet(s.as_bytes()).await;
                }
            }
        }
        Err(e) => {
            let mut s: String<64> = String::new();
            let _ = writeln!(s, "Volume Error: {:?}\r\n", e);
            let _ = class.write_packet(s.as_bytes()).await;
        }
    }

    let _ = class.write_packet("--- Test Complete ---\r\n".as_bytes()).await;
    loop {
        Timer::after_secs(10).await;
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, embassy_rp::usb::Driver<'static, embassy_rp::peripherals::USB>>) {
    usb.run().await;
}
