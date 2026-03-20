use esp_idf_hal::{ gpio::{ Output, PinDriver, Pin }, ledc::LedcDriver };

/// - `Usage` mower.set_speed(speed), mower.set_speed_damped(speed)
/// - `damping` = 0-1 damping factor for speed control when using `set_speed_damped()`
pub struct Mower<'d> {
    pwm: LedcDriver<'d>,
    pub enabled: u8,
    max_speed_step: f32,
    pub speed: f32,
}

// Constructor
impl<'d> Mower<'d> {
    pub fn new(pwm: LedcDriver<'d>, enabled: u8, max_speed_step: f32) -> Self {
        Self {
            pwm,
            enabled,
            max_speed_step,
            speed: 0.0,
        }
    }

    /// Set speed of motor.
    /// - `left_speed` - Set speed from 0 to 1
    pub fn set_speed(&mut self, speed: f32) {
        let max_duty = self.pwm.get_max_duty();
        let pwm_duty = (speed.abs() * (max_duty as f32)) as u32;
        self.speed = speed;
        // set motor drive direction
        if speed >= 0.0 {
            // Forward
            let _ = self.pwm.set_duty(pwm_duty);
        }
    }

    /// Set speed of left & right motor.
    /// Uses max_speed_step for linerally dampening motor control
    /// - `speed` Set speed from 0 to 1
    pub fn set_speed_damped(&mut self, target_speed: f32) {

        // Faster decay when returning to zero
        if target_speed == 0.0 {
            let decay_rate = 0.98; // Adjust this (0.95-0.99)
            self.speed *= decay_rate;
        } else {
            // Normal damping for above 0 target
            let delta_left = target_speed - self.speed;
            self.speed += delta_left.clamp(-self.max_speed_step, self.max_speed_step);
        }

        // Set motor drive speed
        let max_duty = self.pwm.get_max_duty();
        let pwm_duty = (self.speed.abs() * (max_duty as f32)) as u32;
        let _ = self.pwm.set_duty(pwm_duty);
    }
}
