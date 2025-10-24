use esp_idf_hal::i2c::I2cDriver;
use log::info;

pub fn scan_i2c_bus(i2c: &mut I2cDriver) {
    let mut devices = vec![];
    let mut buf = [];
    for addr in 0x00..=0x7F {
        if i2c.write(addr, &buf, 1000).is_ok() {
            devices.push(addr);
            info!("Device found at -> {:#0x?}",addr);
        }
    }
    info!("{} i2c devices found -> {:#0x?}", devices.len(), devices);
}