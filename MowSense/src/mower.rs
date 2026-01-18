use esp_idf_hal::{ gpio::{ Output, PinDriver, Pin }, ledc::LedcDriver };

///
/// - `damping` = 0-1 damping factor for speed control when using `set_speed_damped()`
pub struct Mower<'d, P1: Pin, P2: Pin> {
    pwm: LedcDriver<'d>,
    l_en: PinDriver<'d, P2, Output>,
    r_en: PinDriver<'d, P1, Output>,
    max_speed_step: f32,
    pub speed: f32,
}

// Constructor
impl<'d, P1: Pin, P2: Pin> Mower<'d, P1, P2> {
    pub fn new(
        pwm: LedcDriver<'d>,
        l_en: PinDriver<'d, P2, Output>,
        r_en: PinDriver<'d, P1, Output>,
        max_speed_step: f32
    ) -> Self {
        Self {
            pwm,
            l_en,
            r_en,
            max_speed_step,
            speed: 0.0,
        }
    }

    /// Set speed of motor. Negative values makes motor spin in reverse
    /// - `left_speed` - Set speed from -1 to 1
    pub fn set_speed(&mut self, speed: f32) {
        // set motor drive direction
        if speed >= 0.0 {
            // Forward
            let _ = self.l_en.set_low();
            let _ = self.r_en.set_high();
        } else {
            // Backwards
            let _ = self.l_en.set_high();
            let _ = self.r_en.set_low();
        }

        self.speed = speed;

        // Set pwm duty / motor speed
        let max_duty = self.pwm.get_max_duty();
        let pwm_duty = (speed.abs() * (max_duty as f32)) as u32;
        let _ = self.pwm.set_duty(pwm_duty);
    }

    /// Set speed of left & right motor.
    /// Negative values makes motor spin in reverse.
    /// Uses max_speed_step for linerally dampening motor control
    /// - `speed` Set speed from -1 to 1
    pub fn set_speed_damped(&mut self, target_speed: f32) {
        let delta_left = target_speed - self.speed;

        self.speed += delta_left.clamp(-self.max_speed_step, self.max_speed_step);

        // set motor drive direction
        if self.speed >= 0.0 {
            // Forward
            let _ = self.l_en.set_low();
            let _ = self.r_en.set_high();
        } else {
            // Backwards
            let _ = self.l_en.set_high();
            let _ = self.r_en.set_low();
        }

        // Set motor drive speed
        let max_duty = self.pwm.get_max_duty();
        let pwm_duty = (self.speed.abs() * (max_duty as f32)) as u32;
        let _ = self.pwm.set_duty(pwm_duty);

    }
}
