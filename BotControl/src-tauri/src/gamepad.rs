use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use gilrs::{Axis, Button, Event, EventType, Gilrs};
use log::{debug, info};

static mut CONTROL_BUF: [u8; 13] = [0; 13];

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum RobotMode {
    MANUAL = 0,
    AUTOMATIC = 1,
}
impl RobotMode {
    pub fn next(self) -> Self {
        let current = self as u8;
        let next = (current + 1) % 2;
        match next {
            0 => RobotMode::MANUAL,
            1 => RobotMode::AUTOMATIC,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlInputs {
    pub throttle: f32, // 0.0 to 1.0
    pub steering: f32, // -1.0 to 1.0
    pub mode: RobotMode,
}

impl ControlInputs {
    pub fn start(udp_tx: mpsc::Sender<&'static [u8]>) {
        let mut inputs = ControlInputs {
            throttle: 0.0,
            steering: 0.0,
            mode: RobotMode::MANUAL,
        };
        let mut last_inputs = inputs;

        let mut gilrs = Gilrs::new().unwrap();
        // Iterate over all connected gamepads
        for (_id, gamepad) in gilrs.gamepads() {
            println!("{} is {:?}", gamepad.name(), gamepad.power_info());
        }
        const loop_interval: f32 = 1.0 / 60.0; // In seconds

        let mut active_gamepad = None;
        let mut last_loop = Instant::now();

        loop {
            if last_loop.elapsed() <= Duration::from_secs_f32(loop_interval) {
                continue;
            }
            // Examine new events
            while let Some(Event {
                id, event, time, ..
            }) = gilrs.next_event()
            {
                //debug!("{:?} New event from {}: {:?}", time, id, event);

                match event {
                    EventType::ButtonChanged(button, value, _) => {
                        // ButtonChanged(Select, 0.0, Code(EvCode(EvCode { kind: 1, code: 314 })))
                        match button {
                            Button::Select => {
                                inputs.mode = inputs.mode.next();
                            }
                            Button::RightTrigger2 => {
                                inputs.throttle = value;
                            }
                            _ => {}
                        }
                    }
                    EventType::AxisChanged(axis, value, _) => match axis {
                        Axis::LeftStickX => {
                            inputs.steering = value;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            if inputs != last_inputs {
                last_inputs = inputs;
                let ctrl_packet = inputs.to_packet();
                debug!("CTRL_PACKET -> {:?}", &ctrl_packet);
                unsafe {
                    CONTROL_BUF = ctrl_packet;
                    udp_tx.send(&CONTROL_BUF).ok();
                }
            }

            // You can also use cached gamepad state
            if let Some(gamepad) = active_gamepad.map(|id| gilrs.gamepad(id)) {
                if gamepad.is_pressed(Button::South) {
                    println!("Button South is pressed (XBox - A, PS - X)");
                }
            }

            last_loop = Instant::now();
        }
    }

    /// This function creates Control input packets ready for sending.
    pub fn to_packet(&self) -> [u8; 13] {
        let mut packet = [0u8; 13];
        // HEADER
        packet[0..4].copy_from_slice(b"CTRL");
        packet[4..8].copy_from_slice(&self.throttle.to_le_bytes());
        packet[8..12].copy_from_slice(&self.steering.to_le_bytes());
        packet[12] = self.mode as u8;

        packet
    }
}
