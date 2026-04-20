#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;
use {panic_probe as _};

use fsw_lib::drivers::stepper::Stepper;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    
    // FIXME(config): Verify stepper pins for your board
    let step_pin = Output::new(p.PIN_15, Level::Low);
    let dir_pin = Output::new(p.PIN_16, Level::Low);
    let en_pin = Output::new(p.PIN_14, Level::High); // Start disabled

    // Pass the concrete embassy Output types to the generic Stepper driver
    let mut stepper = Stepper::new(step_pin, dir_pin, Some(en_pin));

    loop {
        defmt::info!("Enabling motor...");
        stepper.enable();
        Timer::after_millis(100).await;

        defmt::info!("Moving Forward 200 steps (Ramped)...");
        stepper.move_ramped(200, 1000, 50).await; 
        
        Timer::after_secs(1).await;

        defmt::info!("Moving Reverse 200 steps (Ramped)...");
        stepper.move_ramped(-200, 500, 50).await; 
        
        Timer::after_secs(1).await;

        defmt::info!("Current Position: {}", stepper.get_position());

        defmt::info!("Disabling motor...");
        stepper.disable();
        Timer::after_secs(5).await;
    }
}
