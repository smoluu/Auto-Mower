use anyhow::{ Ok };
use esp_idf_hal::i2c::{ I2cDriver };
use log::info;
const PI: f32 = 3.141592;
#[derive(Clone)]
/// # Fields
/// - `x` (`f32`) - West.
/// - `y` (`f32`) - Down.
/// - `z` (`f32`) - South.
/// ```
pub struct GY273Reading {
    pub heading: f32,
    pub tilt_compensated_heading: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub struct GY273 {
    addr: u8,
}
impl GY273 {
    pub fn new(addr: u8) -> GY273 {
        GY273 {
            addr: addr,
        }
    }
    pub fn configure(&mut self, i2c: &mut I2cDriver) -> anyhow::Result<()> {
        /*  Register 0x09: GY-273 Clone Control Register 1
        Bits 0-1: Mode (01 = continuous)
        Bits 2-3: Oversampling (00 = 512)
        Bits 4-5: Full-scale range (10 = 8G)
        Bits 6-7: Output rate (10 = 200 Hz)
        Configure for continuous mode, 200 Hz, 8 Gauss range, 512 oversampling
        */
        i2c.write(self.addr, &[0x0a, 0b10_10_00_11], 100)?;
        /*Register 0x0B
        SOFT_RST=0 (no reset), SELF_TEST=0 (disabled)
        RNG<1:0>=10 (8 Gauss),
        SET/RESET MODE<1:0>=00 (set and reset on)
         */
        i2c.write(self.addr, &[0x0b, 0b0000_10_00], 100)?;

        Ok(())
    }

    pub fn read(&mut self, i2c: &mut I2cDriver) -> GY273Reading {
        let mut buf = [0u8; 6];
        i2c.write_read(self.addr, &[0x01], &mut buf, 100);

        let x = i16::from_le_bytes([buf[0], buf[1]]) as f32;
        let y = i16::from_le_bytes([buf[2], buf[3]]) as f32;
        let z = i16::from_le_bytes([buf[4], buf[5]]) as f32;

        let heading = (x.atan2(-z) * 180.0) / PI;
        let heading = (heading + 360.0) % 360.0;

        GY273Reading { heading: heading, x, y, z, tilt_compensated_heading: 0.0 }
    }
}

impl GY273Reading {


    pub fn tilt_compensate(&mut self, acc_pitch: f32, acc_roll:f32) {
        todo!()
    }
}
