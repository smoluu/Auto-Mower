use esp_idf_hal::{ gpio::{Output, PinDriver, Pin }, ledc::LedcDriver};

pub struct Drive<'d, L1: Pin, L2: Pin, R1: Pin, R2: Pin> {
    left_pwm: LedcDriver<'d>,
    right_pwm: LedcDriver<'d>,
    left_in1: PinDriver<'d, L1, Output>,
    left_in2: PinDriver<'d, L2, Output>,
    right_in1: PinDriver<'d, R1, Output>,
    right_in2: PinDriver<'d, R2, Output>,
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
        }
    }

    /// Set speed of left & right motors. Negative values makes motors spin in reverse
    /// - `left_speed` - Set speed from -1 to 1
    ///- `right_speed` - Set speed from -1 to 1
    pub fn set_speed(&mut self, left_speed: f32, right_speed: f32) {
        // set motor drive directions
        if left_speed <= 0.0 {
            self.left_in1.set_high();
            self.left_in2.set_low();
        } else {
            self.left_in1.set_low();
            self.left_in2.set_high();
        }
        if right_speed <= 0.0 {
            self.right_in1.set_high();
            self.right_in2.set_low();
        } else {
            self.right_in1.set_low();
            self.right_in2.set_high();
        }

        let max_duty = self.left_pwm.get_max_duty();

        let left_duty = (left_speed.abs() * (max_duty as f32)) as u32;
        let right_duty = (right_speed.abs() * (max_duty as f32)) as u32;

        // Controlling motor drivers PWM duty 0-1023 on 10Bit
        self.left_pwm.set_duty(left_duty);
        self.right_pwm.set_duty(right_duty);
    }
}
