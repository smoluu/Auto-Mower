use esp_idf_hal::{ gpio::{ Output, PinDriver, Pin }, ledc::LedcDriver };

///
/// - `damping` = 0-1 damping factor for speed control when using `set_speed_damped()`
pub struct Drive<'d, L1: Pin, L2: Pin, R1: Pin, R2: Pin> {
    left_pwm: LedcDriver<'d>,
    right_pwm: LedcDriver<'d>,
    left_in1: PinDriver<'d, L1, Output>,
    left_in2: PinDriver<'d, L2, Output>,
    right_in1: PinDriver<'d, R1, Output>,
    right_in2: PinDriver<'d, R2, Output>,
    pub max_speed_delta: f32,
    pub left_speed: f32,
    pub right_speed: f32,
}

// Constructor
impl<'d, L1: Pin, L2: Pin, R1: Pin, R2: Pin> Drive<'d, L1, L2, R1, R2> {
    pub fn new(
        left_pwm: LedcDriver<'d>,
        right_pwm: LedcDriver<'d>,
        left_in1: PinDriver<'d, L1, Output>,
        left_in2: PinDriver<'d, L2, Output>,
        right_in1: PinDriver<'d, R1, Output>,
        right_in2: PinDriver<'d, R2, Output>,
    ) -> Self {
        Self {
            left_in1,
            left_in2,
            right_in1,
            right_in2,
            left_pwm,
            right_pwm,
            max_speed_delta: 1.0,
            left_speed: 0.0,
            right_speed: 0.0,
        }
    }

    /// Set speed of left & right motors. Negative values makes motors spin in reverse
    /// - `left_speed` - Set speed from -1 to 1
    ///- `right_speed` - Set speed from -1 to 1
    pub fn set_speed(&mut self, left_speed: f32, right_speed: f32) {
        // set motor drive directions
        if left_speed <= 0.0 {
            let _ = self.left_in1.set_high();
            let _ = self.left_in2.set_low();
        } else {
            let _ = self.left_in1.set_low();
            let _ = self.left_in2.set_high();
        }
        if right_speed <= 0.0 {
            let _ = self.right_in1.set_high();
            let _ = self.right_in2.set_low();
        } else {
            let _ = self.right_in1.set_low();
            let _ = self.right_in2.set_high();
        }

        self.left_speed = left_speed;
        self.right_speed = right_speed;
        
        // set motor drive speed
        let max_duty = self.left_pwm.get_max_duty();

        let left_duty = (left_speed.abs() * (max_duty as f32)) as u32;
        let right_duty = (right_speed.abs() * (max_duty as f32)) as u32;

        let _ = self.left_pwm.set_duty(left_duty);
        let _ = self.right_pwm.set_duty(right_duty);
    }

    /// Set speed of left & right motors.
    /// Negative values makes motors spin in reverse.
    /// Uses max_speed_delta for linerally dampening motor control
    /// - `left_speed` Set speed from -1 to 1
    /// - `right_speed` Set speed from -1 to 1
    pub fn set_speed_damped(
        &mut self,
        left_target_speed: f32,
        right_target_speed: f32,
    ) {
        let delta_left = left_target_speed - self.left_speed;
        let delta_right = right_target_speed - self.right_speed;
        
        self.left_speed += delta_left.clamp(-self.max_speed_delta, self.max_speed_delta);
        self.right_speed += delta_right.clamp(-self.max_speed_delta, self.max_speed_delta);

        // Set motor drive directions
        if left_target_speed <= 0.0 {
            let _ = self.left_in1.set_high();
            let _ = self.left_in2.set_low();
        } else {
            let _ = self.left_in1.set_low();
            let _ = self.left_in2.set_high();
        }
        if right_target_speed <= 0.0 {
            let _ = self.right_in1.set_high();
            let _ = self.right_in2.set_low();
        } else {
            let _ = self.right_in1.set_low();
            let _ = self.right_in2.set_high();
        }

        // Set motor drive speed
        let max_duty = self.left_pwm.get_max_duty();

        let left_duty = (self.left_speed.abs() * (max_duty as f32)) as u32;
        let right_duty = (self.right_speed.abs() * (max_duty as f32)) as u32;

        let _ = self.left_pwm.set_duty(left_duty);
        let _ = self.right_pwm.set_duty(right_duty);
    }
}
