#[derive(Debug)]
pub enum Packet {
    Ctrl {
        throttle: f32,
        steering: f32,
        mode: u8,
    },
    Keepalive {},
    Unknown {
        raw: Vec<u8>,
    },
    // PNTT = POSITION, NAVIGATION, TEMPERATURE, TIMING
    PNTT {
        heading: f32,
        roll: f32,
        pitch: f32,
        temp_c_0: f32,
        acc_total: f32,
        pressure: f32,
        temp_c_1: f32,
        timestamp_us: i64,
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
            b"KEEP" => { Some(Packet::Keepalive {}) }

            b"CTRL" if data.len() == 13 => {
                let throttle = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let steering = f32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let mode = data[12];

                Some(Packet::Ctrl { throttle, steering, mode })
            }
            b"PNTT" if data.len() == 40 => {
                // All floats are little-endian
                let heading = f32::from_le_bytes(data[4..8].try_into().unwrap());
                let roll = f32::from_le_bytes(data[8..12].try_into().unwrap());
                let pitch = f32::from_le_bytes(data[12..16].try_into().unwrap());
                let temp_c_0 = f32::from_le_bytes(data[16..20].try_into().unwrap());
                let acc_total = f32::from_le_bytes(data[20..24].try_into().unwrap());
                let pressure = f32::from_le_bytes(data[24..28].try_into().unwrap());
                let temp_c_1 = f32::from_le_bytes(data[28..32].try_into().unwrap());

                let timestamp_us = i64::from_le_bytes(data[32..40].try_into().unwrap());

                Some(Packet::PNTT {
                    heading,
                    roll,
                    pitch,
                    temp_c_0,
                    acc_total,
                    pressure,
                    temp_c_1,
                    timestamp_us,
                })
            }

            _ => Some(Packet::Unknown { raw: data.to_vec() }),
        }
    }

    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Packet::PNTT {
                heading,
                roll,
                pitch,
                temp_c_0,
                acc_total,
                pressure,
                temp_c_1,
                timestamp_us,
            } => {
                let mut buf = Vec::with_capacity(4 + 7 * 4 + 8);
                buf.extend_from_slice(b"PNTT");

                buf.extend_from_slice(&heading.to_le_bytes()); // 4
                buf.extend_from_slice(&roll.to_le_bytes());
                buf.extend_from_slice(&pitch.to_le_bytes());
                buf.extend_from_slice(&temp_c_0.to_le_bytes());
                buf.extend_from_slice(&acc_total.to_le_bytes());
                buf.extend_from_slice(&pressure.to_le_bytes());
                buf.extend_from_slice(&temp_c_1.to_le_bytes());

                buf.extend_from_slice(&timestamp_us.to_le_bytes());

                Some(buf)
            }
            Packet::Unknown { raw } => Some(raw.clone()),
            _ => { None }
        }
    }

    fn xor_checksum(data: &[u8]) -> u8 {
        data.iter().fold(0u8, |acc, &byte| acc ^ byte)
    }
}
