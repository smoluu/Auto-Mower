use esp_idf_hal::i2c::I2cDriver;
use log::info;

pub fn scan_i2c_bus(i2c: &mut I2cDriver, start_addr: u8, end_addr: u8, timeout: u32) {
    let mut devices = vec![];
    let mut buf = [];
    for addr in 0x03..=0x77 {
        if i2c.write(addr, &buf, timeout).is_ok() {
            devices.push(addr);
            info!("Device found at -> {}",addr);
        }
    }

    info!("{} i2c devices found -> {:?}", devices.len(), devices);
}