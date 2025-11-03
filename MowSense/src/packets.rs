#[derive(Debug)]
pub enum Packet {
    Ctrl {
        throttle: f32,
        steering: f32,
        mode: u8,
    },
    Unknown {
        raw: Vec<u8>,
    },
}
impl Packet {
    /// Parse a raw UDP payload into a `Packet`.
    /// Returns `None` if the buffer is too short.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        match &data[0..4] {
            b"CTRL" if data.len() >= 13 => {
                let throttle = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let steering = f32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let mode = data[12];

                Some(Packet::Ctrl { throttle, steering, mode })
            }
            _ => Some(Packet::Unknown { raw: data.to_vec() }),
        }
    }
}
