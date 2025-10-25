use anyhow::{ Ok };
use esp_idf_hal::i2c::{ I2cDriver };
use log::info;
const PI: f32 = 3.141592;

pub struct MPUReading {
    pub roll: f32,
    pub pitch: f32,
    pub temperature_c: f32,
    pub acc_total: f32
}

pub struct MPU {
    addr: u8,
    acc_x_offset: f32,
    acc_y_offset: f32,
    acc_z_offset: f32,
    gyro_x_offset: f32,
    gyro_y_offset: f32,
    gyro_z_offset: f32,
}
impl MPU {
    pub fn new(addr: u8) -> MPU {
        MPU {
            addr: addr,
            acc_x_offset: 0.0,
            acc_y_offset: 0.0,
            acc_z_offset: 0.0,
            gyro_x_offset: 0.0,
            gyro_y_offset: 0.0,
            gyro_z_offset: 0.0,
        }
    }
    pub fn configure(&mut self, i2c: &mut I2cDriver) -> anyhow::Result<()> {
        // MPU Configuration
        // Reset / Wake up
        i2c.write(self.addr, &[0x6b, 0x00], 100)?;
        // Digital low pass filter
        i2c.write(self.addr, &[0x1a, 0x06], 100)?;
        // Gyro sensivity   250 deg/s -> 0x00, 500 deg/s -> 0x08, 1000 deg/s -> 0x10, 2000 deg/s -> 0x18
        i2c.write(self.addr, &[0x1b, 0x00], 100)?;
        // Acceleration sensivity +8g
        i2c.write(self.addr, &[0x1c, 0x10], 100)?;

        // Calibration
        let mut acc_x_sum = 0.0;
        let mut acc_y_sum = 0.0;
        let mut acc_z_sum = 0.0;
        let mut gyro_x_sum = 0.0;
        let mut gyro_y_sum = 0.0;
        let mut gyro_z_sum = 0.0;
        for _ in 0..100 {
            let mut buf = [0u8; 14];
            i2c.write_read(self.addr, &[0x3b], &mut buf, 100)?;
            let acc_x = (i16::from_be_bytes([buf[0], buf[1]]) as f32) / 4096.0;
            let acc_y = (i16::from_be_bytes([buf[2], buf[3]]) as f32) / 4096.0;
            let acc_z = (i16::from_be_bytes([buf[4], buf[5]]) as f32) / 4096.0;
            let gyro_x = (i16::from_be_bytes([buf[8], buf[9]]) as f32) / 65.5;
            let gyro_y = (i16::from_be_bytes([buf[10], buf[11]]) as f32) / 65.5;
            let gyro_z = (i16::from_be_bytes([buf[12], buf[13]]) as f32) / 65.5;
            acc_x_sum += acc_x;
            acc_y_sum += acc_y;
            acc_z_sum += acc_z;
            gyro_x_sum += gyro_x;
            gyro_y_sum += gyro_y;
            gyro_z_sum += gyro_z;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.acc_x_offset = acc_x_sum / 100.0;
        self.acc_y_offset = acc_y_sum / 100.0 + 1.0;
        self.acc_z_offset = acc_z_sum / 100.0;
        self.gyro_x_offset = gyro_x_sum / 100.0;
        self.gyro_y_offset = gyro_y_sum / 100.0;
        self.gyro_z_offset = gyro_z_sum / 100.0;
        Ok(())
    }

    pub fn read(&mut self, i2c: &mut I2cDriver) -> MPUReading {
        let mut buf = [0u8; 14];
        i2c.write_read(self.addr, &[0x3b], &mut buf, 100);

        let acc_x = (i16::from_be_bytes([buf[0], buf[1]]) as f32) / 4096.0 - self.acc_x_offset;
        let acc_y = (i16::from_be_bytes([buf[2], buf[3]]) as f32) / 4096.0 - self.acc_y_offset;
        let acc_z = (i16::from_be_bytes([buf[4], buf[5]]) as f32) / 4096.0 - self.acc_z_offset;
        let temperature = i16::from_be_bytes([buf[6], buf[7]]) as f32;
        let gyro_x = (i16::from_be_bytes([buf[8], buf[9]]) as f32) / 65.5 - self.gyro_x_offset;
        let gyro_y = (i16::from_be_bytes([buf[10], buf[11]]) as f32) / 65.5 - self.gyro_y_offset;
        let gyro_z = (i16::from_be_bytes([buf[12], buf[13]]) as f32) / 65.5 - self.gyro_z_offset;

        let temperature_c = temperature / 340.0 + 36.53 - 16.9;

        // Remap accelerometer axes to robot frame: X (forward), Y (left), Z (up)
        let acc_x_robot = -acc_z; // Sensor Z -> Robot X (forward)
        let acc_y_robot = -acc_x; // Sensor X -> Robot Y (left)
        let acc_z_robot = -acc_y; // Sensor Y -> Robot Z (up, negate gravity)

        // Remap gyroscope axes to robot frame
        let gyro_x_robot = -gyro_z; // Sensor Z -> Robot X
        let gyro_y_robot = -gyro_x; // Sensor X -> Robot Y
        let gyro_z_robot = -gyro_y; // Sensor Y -> Robot Z

        // Total acceleration in robot frame
        let acc_total = (
            acc_x_robot * acc_x_robot +
            acc_y_robot * acc_y_robot +
            acc_z_robot * acc_z_robot
        ).sqrt();

        // Normalize acceleration vectors
        let norm = (
            acc_x_robot * acc_x_robot +
            acc_y_robot * acc_y_robot +
            acc_z_robot * acc_z_robot
        ).sqrt();

        let (acc_x_robot, acc_y_robot, acc_z_robot) = if norm > 1e-6 {
            (acc_x_robot / norm, acc_y_robot / norm, acc_z_robot / norm)
        } else {
            (acc_x_robot, acc_y_robot, acc_z_robot)
        };

        // Roll and pitch in robot frame
        let roll =
            (acc_y_robot.atan2(f32::sqrt(acc_x_robot * acc_x_robot + acc_z_robot * acc_z_robot)) *
                180.0) /
            PI;
        let pitch =
            (acc_x_robot.atan2(f32::sqrt(acc_y_robot * acc_y_robot + acc_z_robot * acc_z_robot)) *
                180.0) /
            PI;


        MPUReading { roll, pitch, temperature_c, acc_total }
    }
}
