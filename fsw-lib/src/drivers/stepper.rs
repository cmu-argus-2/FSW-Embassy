use embassy_rp::gpio::Output;
use embassy_time::{Duration, Timer};

/*
 * Async Stepper Motor Driver with Linear Ramping
 * 
 * Stepper motors have inertia. If you try to jump from 0 to 1000 steps/sec 
 * instantly, the motor will "stall" (vibrate without moving).
 * 
 * This driver implements a linear "ramp" that gradually increases the 
 * speed at the start of a move and gradually decreases it at the end.
 */

pub struct Stepper<'d> {
    step_pin: Output<'d>,
    dir_pin: Output<'d>,
    enable_pin: Option<Output<'d>>,
    current_step: i64, // Track position relative to boot
}

impl<'d> Stepper<'d> {
    pub fn new(
        step_pin: Output<'d>,
        dir_pin: Output<'d>,
        enable_pin: Option<Output<'d>>,
    ) -> Self {
        Self {
            step_pin,
            dir_pin,
            enable_pin,
            current_step: 0,
        }
    }

    /// Powers on the motor driver.
    pub fn enable(&mut self) {
        if let Some(ref mut en) = self.enable_pin {
            en.set_low(); // Most drivers (like DRV8825) use active-low enable
        }
    }

    /// Powers off the motor driver to save satellite battery.
    pub fn disable(&mut self) {
        if let Some(ref mut en) = self.enable_pin {
            en.set_high();
        }
    }

    /// Moves the motor with a linear speed ramp.
    /// 
    /// - `steps`: Number of steps to move (negative for reverse).
    /// - `target_speed_hz`: The "cruise" speed in steps per second.
    /// - `accel_steps`: How many steps to spend accelerating/decelerating.
    pub async fn move_ramped(&mut self, steps: i32, target_speed_hz: u32, accel_steps: u32) {
        if steps == 0 || target_speed_hz == 0 { return; }

        // Set direction pin
        if steps > 0 {
            self.dir_pin.set_high();
        } else {
            self.dir_pin.set_low();
        }

        let total_steps = steps.unsigned_abs();
        let actual_accel_steps = accel_steps.min(total_steps / 2);
        
        // Start at a slow, safe speed
        let start_speed_hz = 100u32; 
        let speed_range = target_speed_hz.saturating_sub(start_speed_hz);

        for i in 0..total_steps {
            let current_speed = if i < actual_accel_steps {
                // Acceleration: Linearly increase speed
                let speed_inc = (speed_range as u64 * i as u64) / actual_accel_steps as u64;
                start_speed_hz + speed_inc as u32
            } else if i > (total_steps - actual_accel_steps) {
                // Deceleration: Linearly decrease speed
                let decel_index = i - (total_steps - actual_accel_steps);
                let speed_dec = (speed_range as u64 * decel_index as u64) / actual_accel_steps as u64;
                target_speed_hz - speed_dec as u32
            } else {
                // Constant speed ("Cruise")
                target_speed_hz
            };

            // Safety floor: 10Hz minimum to avoid infinite delays
            let speed_floor = current_speed.max(10);

            // Calculate timing for this specific pulse
            let delay = Duration::from_micros(1_000_000 / (speed_floor as u64 * 2));
            
            // Generate the step pulse (High-Low toggle)
            self.step_pin.set_high();
            Timer::after(delay).await;
            self.step_pin.set_low();
            Timer::after(delay).await;

            // Keep track of our global position
            if steps > 0 { self.current_step += 1; } else { self.current_step -= 1; }
        }
    }

    /// Get the current step count relative to when the driver was initialized.
    pub fn get_position(&self) -> i64 {
        self.current_step
    }

    /// Manually override the internal position (useful after homing sensors).
    pub fn set_position(&mut self, pos: i64) {
        self.current_step = pos;
    }
}
