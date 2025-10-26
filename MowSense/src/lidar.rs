use esp_idf_hal::uart::{ Uart };

const MAX_PACKET_SIZE: usize = 104;
const POINT_DATA_LEN: usize = 3;
const HEADER_BYTE: u8 = 0x54;

struct LidarPacket {
    speed: u16, // Rotational speed in degrees/s
    start_angle: u16,
    points: Vec<LidarPoints>,
    end_angle: u16,
    timestamp: u16,
}
impl LidarPacket {
    pub fn parse(packet: &[u8]) -> LidarPacket {
        // Ignore header
        let point_len = (packet[1] & 0b0001_1111) as usize;
        let speed = u16::from_le_bytes([packet[2], packet[3]]);
        let start_angle = u16::from_le_bytes([packet[4], packet[5]]);
        let mut points = Vec::with_capacity(point_len);
        for i in 0..point_len {
            let offset = 6 + i * 3;

            let distance = u16::from_le_bytes([packet[offset], packet[offset + 1]]);
            let intensity = u8::from_le_bytes([packet[offset + 2]]);
            points.push(LidarPoints { distance, intensity });
        }
        let end_angle = u16::from_le_bytes([packet[6 + point_len * 3], packet[7 + point_len * 3]]);
        let timestamp = u16::from_le_bytes([packet[8 + point_len * 3], packet[9 + point_len * 3]]);

        LidarPacket {
            speed,
            start_angle,
            points,
            end_angle,
            timestamp,
        }
    }
}

struct LidarPoints {
    distance: u16,
    intensity: u8,
}

struct Lidar {}

impl Lidar {
    pub fn read(
        uart: &mut esp_idf_hal::uart::UartRxDriver
    ) -> Result<Vec<LidarPacket>, anyhow::Error> {
        let mut temp_buf = [0u8; 4096];
        let mut recv_buf: Vec<u8> = Vec::new();
        let mut lidar_packets: Vec<LidarPacket> = vec![];

        let len = uart
            .read(&mut temp_buf, 100)
            .map_err(|e| anyhow::anyhow!("Error reading uart: {e}"))?;

        recv_buf.extend_from_slice(&temp_buf[..len]);

        let mut i = 0;
        while i + MAX_PACKET_SIZE <= recv_buf.len() {
            // Find full packet
            if recv_buf[i] == HEADER_BYTE {
                if i + 1 >= recv_buf.len() {
                    // check ver_len
                    let point_len = (recv_buf[i + 1] & 0b0001_1111) as usize;
                    let packet_len = 11 + 3 * point_len;
                    if i + packet_len - 1 >= recv_buf.len() {
                        let packet = LidarPacket::parse(&recv_buf[i..i + packet_len]);
                        lidar_packets.push(packet);
                    } else {
                        break; // Not enought bytes to read whole packet
                    }
                } else {
                    break; // Not enought bytes to read ver_len
                }
            } else {
                i += 1; // Skip byte until header is found
            }
        }

        // Remove processed bytes from front
        if i > 0 {
            recv_buf.drain(..i);
        }

        // Keep buffer from growing infinitely
        if recv_buf.len() > 512 {
            recv_buf.clear();
        }
        Ok(lidar_packets)
    }
}
