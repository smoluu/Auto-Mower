use std::{thread::sleep, time::Duration};
use esp_idf_hal::{ledc::{LedcDriver, LedcTimerDriver}, units::Hertz};

pub fn play(
    buzzer: &mut LedcDriver,
    timer: &mut LedcTimerDriver<'_, esp_idf_hal::ledc::TIMER1>,
    melody: &'static [(u32, u32)]
) {
    for &(freq, duration) in melody {
        if freq == 0 {
            buzzer.set_duty(0).unwrap();
        } else {
            timer.set_frequency(Hertz(freq)).unwrap();
            buzzer.set_duty(256).unwrap(); // 50% duty
        }
        sleep(Duration::from_millis(duration as u64));
    }
    buzzer.set_duty(0).unwrap(); // Final silence
}

pub static STARTUP: &[(u32, u32)] = &[
    (523,  200),  // C5
    (659,  200),  // E5
    (784,  300),  // G5
    (1047, 400),  // C6  – warm resolve
    (0,    100),
];