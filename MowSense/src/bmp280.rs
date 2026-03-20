use esp_idf_hal::i2c::{ I2cDriver, I2cError };
use log::info;

pub struct Bmp280Reading {
    pub pressure: f32,
    pub temperature: f32,
}

pub struct Bmp280 {
    addr: u8,
    pub dig_t1: u16,
    pub dig_t2: i16,
    pub dig_t3: i16,
    pub dig_p1: u16,
    pub dig_p2: i16,
    pub dig_p3: i16,
    pub dig_p4: i16,
    pub dig_p5: i16,
    pub dig_p6: i16,
    pub dig_p7: i16,
    pub dig_p8: i16,
    pub dig_p9: i16,
    pub t_fine: i32,
}
impl Bmp280 {
    pub fn new(addr: u8) -> Bmp280 {
        Bmp280 {
            addr: addr,
            dig_t1: 0,
            dig_t2: 0,
            dig_t3: 0,
            dig_p1: 0,
            dig_p2: 0,
            dig_p3: 0,
            dig_p4: 0,
            dig_p5: 0,
            dig_p6: 0,
            dig_p7: 0,
            dig_p8: 0,
            dig_p9: 0,
            t_fine: 0,
        }
    }
    pub fn configure(&mut self, i2c: &mut I2cDriver) -> anyhow::Result<()> {
        // BMP280 Configuration

        /* Register 0xF4: BMP280 Control Measurement Register (ctrl_meas)
        Bits 7-5: osrs_t Temperature oversampling(001 = 1x, 010 = ×2) increases latency by 2ms * x1
        Bits 4-2: osrs_p pressure oversampling (100 = 8x, 101 = ×16) increases latency by 2ms * x1
        Bits 1-0: mode (11 = normal mode, continuous sampling)
        Configure for temperature oversampling ×1, pressure oversampling ×8, normal mode
        */
        i2c.write(self.addr, &[0xf4, 0b001_100_11], 100)?;

        /* Register 0xF5: BMP280 Configuration Register (config)
        Bits 7-5: t_sb Standby time (000 = 0ms, 001 = 62.5, 010 = 125ms)
        Bits 4-2: filter (010 = IIR filter coefficient 4)
        Bit 0: spi3w_en (0 = I2C mode, SPI disabled)
        Bits 1, 2: Reserved (0)
        Configure for 0ms standby, filter coefficient 4, I2C mode
            */
        i2c.write(self.addr, &[0xf5, 0b000_010_0_0], 100)?;

        /* Read BMP280 calibration coefficients (registers 0x88–0x9F)
        dig_T1 (u16, 0x88-0x89): Temperature coefficient 1
        dig_T2 (i16, 0x8A-0x8B): Temperature coefficient 2
        dig_T3 (i16, 0x8C-0x8D): Temperature coefficient 3
        dig_P1 (u16, 0x8E-0x8F): Pressure coefficient 1
        dig_P2–dig_P9 (i16, 0x90–0x9F): Pressure coefficients 2–9
        Read once at startup for temperature and pressure compensation
            */
        let mut cal_buf = [0u8; 24];
        i2c.write_read(self.addr, &[0x88], &mut cal_buf, 100)?;
        self.dig_t1 = u16::from_le_bytes([cal_buf[0], cal_buf[1]]);
        self.dig_t2 = i16::from_le_bytes([cal_buf[2], cal_buf[3]]);
        self.dig_t3 = i16::from_le_bytes([cal_buf[4], cal_buf[5]]);
        self.dig_p1 = u16::from_le_bytes([cal_buf[6], cal_buf[7]]);
        self.dig_p2 = i16::from_le_bytes([cal_buf[8], cal_buf[9]]);
        self.dig_p3 = i16::from_le_bytes([cal_buf[10], cal_buf[11]]);
        self.dig_p4 = i16::from_le_bytes([cal_buf[12], cal_buf[13]]);
        self.dig_p5 = i16::from_le_bytes([cal_buf[14], cal_buf[15]]);
        self.dig_p6 = i16::from_le_bytes([cal_buf[16], cal_buf[17]]);
        self.dig_p7 = i16::from_le_bytes([cal_buf[18], cal_buf[19]]);
        self.dig_p8 = i16::from_le_bytes([cal_buf[20], cal_buf[21]]);
        self.dig_p9 = i16::from_le_bytes([cal_buf[22], cal_buf[23]]);

        info!("dig_t1: {}", self.dig_t1);
        info!("dig_t2: {}", self.dig_t2);
        info!("dig_t3: {}", self.dig_t3);
        info!("dig_p1: {}", self.dig_p1);
        info!("dig_p2: {}", self.dig_p2);
        info!("dig_p3: {}", self.dig_p3);
        info!("dig_p4: {}", self.dig_p4);
        info!("dig_p5: {}", self.dig_p5);
        info!("dig_p6: {}", self.dig_p6);
        info!("dig_p7: {}", self.dig_p7);
        info!("dig_p8: {}", self.dig_p8);
        info!("dig_p9: {}", self.dig_p9);

        Ok(())
    }

    pub fn read(&mut self, i2c: &mut I2cDriver) -> Result<Bmp280Reading, anyhow::Error> {
        // BMP280 reading
        /* Register 0xF7–0xFC: BMP280 Data Registers
       0xF7–0xF9: Pressure (MSB, LSB, XLSB, 20-bit)
       0xFA–0xFC: Temperature (MSB, LSB, XLSB, 20-bit)
       Read 6 bytes for raw pressure and temperature ADC values
        */
        let mut buf = [0u8; 6];
        match i2c.write_read(self.addr, &[0xf7], &mut buf, 100) {
            Ok(_) => {
                // Combine bytes
                let adc_p =
                    ((buf[0] as u32) << 12) | ((buf[1] as u32) << 4) | ((buf[2] as u32) >> 4);
                let adc_t =
                    ((buf[3] as u32) << 12) | ((buf[4] as u32) << 4) | ((buf[5] as u32) >> 4);

                //info!("BMP280_RAW -> p:{adc_p} t:{adc_t}");

                let temperature: f32 = self.compensate_temperature(adc_t);
                let pressure: f32 = self.compensate_pressure(adc_p);

                Ok(Bmp280Reading { pressure: pressure, temperature: temperature })
            }
            Err(e) => {
                eprintln!("I2C read failed: {:?}", e);

                Err(anyhow::anyhow!(e))
            }
        }
    }

    /* Compensate BMP280 pressure
    Uses calibration coefficients (dig_p1–dig_p9) and t_fine stored in the Bmp280 struct
    Computes pressure using the BMP280 datasheet formula
     */
    fn compensate_pressure(&self, adc_p: u32) -> f32 {
        let adc_p = adc_p as i32;
        let var1 = (self.t_fine as f64) / 2.0 - 64000.0;
        let var2 = (var1 * var1 * (self.dig_p6 as f64)) / 32768.0;
        let var2 = var2 + var1 * (self.dig_p5 as f64) * 2.0;
        let var2 = var2 / 4.0 + (self.dig_p4 as f64) * 65536.0;
        let var1 =
            (((self.dig_p3 as f64) * var1 * var1) / 524288.0 + (self.dig_p2 as f64) * var1) /
            524288.0;
        let var1 = (1.0 + var1 / 32768.0) * (self.dig_p1 as f64);
        if var1 == 0.0 {
            return 0.0; // Avoid division by zero
        }
        let p = 1048576.0 - (adc_p as f64);
        let p = ((p - var2 / 4096.0) * 6250.0) / var1;
        let var1 = ((self.dig_p9 as f64) * p * p) / 2147483648.0;
        let var2 = (p * (self.dig_p8 as f64)) / 32768.0;
        let p = p + (var1 + var2 + (self.dig_p7 as f64)) / 16.0;
        p as f32 // Pa
    }
    /* Compensate BMP280 temperature
    Uses calibration coefficients (dig_t1, dig_t2, dig_t3) stored in the Bmp280 struct
    Computes temperature using the BMP280 datasheet formula
    */
    fn compensate_temperature(&mut self, adc_t: u32) -> f32 {
        let adc_t = adc_t as i32;
        let var1 = (((adc_t >> 3) - ((self.dig_t1 as i32) << 1)) * (self.dig_t2 as i32)) >> 11;
        let var2 =
            (((((adc_t >> 4) - (self.dig_t1 as i32)) * ((adc_t >> 4) - (self.dig_t1 as i32))) >>
                12) *
                (self.dig_t3 as i32)) >>
            14;
        self.t_fine = var1 + var2;
        let t = (self.t_fine * 5 + 128) >> 8;
        (t as f32) / 100.0
    }
}
