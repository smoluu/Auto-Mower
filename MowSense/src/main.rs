use std::{ sync::{ Arc, Mutex }, thread, time::Duration };
use esp_idf_hal::{ prelude::Peripherals, units::Hertz };
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::*,
    nvs::EspDefaultNvsPartition,
    wifi::EspWifi,
};
use esp_idf_hal::i2c::*;

use log::{ self, info, Log };
use esp_idf_sys::*;
use ota::init_ota;

use crate::{ i2c::scan_i2c_bus, ota::start_ota_polling, wifi::wifi_connect };
mod ota;
mod wifi;
mod i2c;
static I2C_Timeout: u32 = 2000;
static PIN_UART0_TX: i32 = 43;
static PIN_UART0_RX: i32 = 44;
static PIN_LED_RGB: i32 = 48;

static PI: f32 = 3.141592;

// Sensor i2c adresses
static SENSOR_ADDR_GY273: u8 = 0x2c;
static SENSOR_ADDR_MPU: u8 = 0x68;

static FIRMWARE_VERSION: &str = "0.0.1"; // This needs to change for firmware to update
static OTA_SERVER_URL: &str = env!("OTA_SERVER_URL");
static OTA_SERVER_POLLING_RATE: u64 = 20;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    // info!("Hello, world!");
    // info!("TESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTEST");
    // info!("TESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTEST");
    // info!("TESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTEST");
    // info!("TESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTEST");
    // info!("TESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTEST");

    let peripherals = Peripherals::take().unwrap();

    //  Wifi setup
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().ok();
    //let mut wifi = EspWifi::new(peripherals.modem, sys_loop.clone(), nvs)?;
    //wifi = wifi_connect(wifi).unwrap();

    let sda = peripherals.pins.gpio8;
    let scl = peripherals.pins.gpio9;

    // I2C configuration
    let config = I2cConfig::new()
        .baudrate(Hertz(100_000)) // 100 kHz
        .sda_enable_pullup(true)
        .scl_enable_pullup(true);
    let mut i2c = I2cDriver::new(peripherals.i2c0, sda, scl, &config)?;

    i2c::scan_i2c_bus(&mut i2c);

    // I2c sensor configuration

    // GY-273 Configuration

    /*  Register 0x09: GY-273 Clone Control Register 1
    Bits 0-1: Mode (01 = continuous)
    Bits 2-3: Oversampling (00 = 512)
    Bits 4-5: Full-scale range (10 = 8G)
    Bits 6-7: Output rate (10 = 200 Hz)
    Configure for continuous mode, 200 Hz, 8 Gauss range, 512 oversampling
    */
    i2c.write(SENSOR_ADDR_GY273, &[0x0a, 0b10_10_00_11], I2C_Timeout)?;
    // Register 0x0B: SOFT_RST=0 (no reset), SELF_TEST=0 (disabled), RNG<1:0>=10 (8 Gauss),
    // SET/RESET MODE<1:0>=00 (set and reset on)
    i2c.write(SENSOR_ADDR_GY273, &[0x0b, 0b0000_10_00], I2C_Timeout)?;

    // MPU Configuration
    // Reset / Wake up
    i2c.write(SENSOR_ADDR_MPU, &[0x6B, 0x00], I2C_Timeout)?;
    // Digital low pass filter
    i2c.write(SENSOR_ADDR_MPU, &[0x1a, 0x06], I2C_Timeout)?;
    // Gyro sensivity   250 deg/s -> 0x00, 500 deg/s -> 0x08, 1000 deg/s -> 0x10, 2000 deg/s -> 0x18
    i2c.write(SENSOR_ADDR_MPU, &[0x1b, 0x00], I2C_Timeout)?;
    // Acceleration sensivity +8g
    i2c.write(SENSOR_ADDR_MPU, &[0x1c, 0x10], I2C_Timeout)?;

    thread::sleep(Duration::from_millis(50));

    // Calibration
    let mut acc_x_sum = 0.0;
    let mut acc_y_sum = 0.0;
    let mut acc_z_sum = 0.0;
    let mut gyro_x_sum = 0.0;
    let mut gyro_y_sum = 0.0;
    let mut gyro_z_sum = 0.0;
    for _ in 0..100 {
        let mut buffer = [0u8; 14];
        i2c.write_read(SENSOR_ADDR_MPU, &[0x3b], &mut buffer, I2C_Timeout)?;
        let acc_x = (i16::from_be_bytes([buffer[0], buffer[1]]) as f32) / 4096.0;
        let acc_y = (i16::from_be_bytes([buffer[2], buffer[3]]) as f32) / 4096.0;
        let acc_z = (i16::from_be_bytes([buffer[4], buffer[5]]) as f32) / 4096.0;
        let gyro_x = (i16::from_be_bytes([buffer[8], buffer[9]]) as f32) / 65.5;
        let gyro_y = (i16::from_be_bytes([buffer[10], buffer[11]]) as f32) / 65.5;
        let gyro_z = (i16::from_be_bytes([buffer[12], buffer[13]]) as f32) / 65.5;
        acc_x_sum += acc_x;
        acc_y_sum += acc_y;
        acc_z_sum += acc_z;
        gyro_x_sum += gyro_x;
        gyro_y_sum += gyro_y;
        gyro_z_sum += gyro_z;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let acc_x_offset = acc_x_sum / 100.0;
    let acc_y_offset = acc_y_sum / 100.0 + 1.0;
    let acc_z_offset = acc_z_sum / 100.0;
    let gyro_x_offset = gyro_x_sum / 100.0;
    let gyro_y_offset = gyro_y_sum / 100.0;
    let gyro_z_offset = gyro_z_sum / 100.0;

    // Check OTA partitions
    info!("Init ota: {:?}", init_ota()?);

    // Start OTA firmware update polling
    // let ota = match start_ota_polling(OTA_SERVER_URL, FIRMWARE_VERSION, OTA_SERVER_POLLING_RATE) {
    //     Ok(_) => {}
    //     Err(e) => panic!("OTA polling start failed => {}", e),
    // };

    loop {
        //print_memory_info();
        // if wifi.is_connected().map_err(|e| anyhow::anyhow!("Connection check failed: {}", e))? {
        //     if let Ok(ip_info) = wifi.sta_netif().get_ip_info() {
        //         info!("Wi-Fi is active. IP: {}", ip_info.ip);
        //     }
        // } else {
        //     info!("Wi-Fi disconnected, attempting reconnect...");
        //     wifi.connect();
        //     while !wifi.is_connected().map_err(|e| anyhow::anyhow!("Reconnect failed: {}", e))? {
        //         thread::sleep(Duration::from_millis(500));
        //     }
        //     let ip_info = wifi.sta_netif().get_ip_info()?;
        //     info!("Reconnected! IP: {}", ip_info.ip);
        // }

        // reading Magnometer data

        let mut buf = [0u8; 6];
        i2c.write_read(SENSOR_ADDR_GY273, &[0x01], &mut buf, I2C_Timeout)?;

        let x = i16::from_le_bytes([buf[0], buf[1]]) as f32;
        let y = i16::from_le_bytes([buf[2], buf[3]]) as f32;
        let z = i16::from_le_bytes([buf[4], buf[5]]) as f32;

        let heading = (y.atan2(-z) * 180.0) / PI;
        let heading_normalized = (heading + 360.0) % 360.0;
        info!("Heading: {:.2}", heading_normalized);

        // Reading MPU data

        let mut buf = [0u8; 14];
        i2c.write_read(SENSOR_ADDR_MPU, &[0x3b], &mut buf, I2C_Timeout)?;

        let acc_x = (i16::from_be_bytes([buf[0], buf[1]]) as f32) / 4096.0 - acc_x_offset;
        let acc_y = (i16::from_be_bytes([buf[2], buf[3]]) as f32) / 4096.0 - acc_y_offset;
        let acc_z = (i16::from_be_bytes([buf[4], buf[5]]) as f32) / 4096.0 - acc_z_offset;
        let temperature = i16::from_be_bytes([buf[6], buf[7]]) as f32;
        let gyro_x = (i16::from_be_bytes([buf[8], buf[9]]) as f32) / 65.5 - gyro_x_offset;
        let gyro_y = (i16::from_be_bytes([buf[10], buf[11]]) as f32) / 65.5 - gyro_y_offset;
        let gyro_z = (i16::from_be_bytes([buf[12], buf[13]]) as f32) / 65.5 - gyro_z_offset;

        let temperature_c = temperature / 340.0 + 36.53 - 16.9;
        // Remap accelerometer axes to robot frame: X (forward), Y (left), Z (up)
        let acc_x_robot = -acc_z; // Sensor Z (backward) -> Robot X (forward)
        let acc_y_robot = -acc_x; // Sensor X (left) -> Robot Y (left)
        let acc_z_robot = -acc_y; // Sensor Y (down) -> Robot Z (up, negate gravity)

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

        info!("Roll: {:.2} Pitch: {:.2} Temp: {:.2}°C", roll, pitch, temperature_c);



        thread::sleep(Duration::from_millis(100));
    }
}
fn print_memory_info() {
    unsafe {
        let free_heap = esp_get_free_heap_size();
        let min_heap = esp_get_minimum_free_heap_size();
        let free_internal = heap_caps_get_free_size(0); // MALLOC_CAP_8BIT | MALLOC_CAP_INTERNAL
        let free_psram = heap_caps_get_free_size(0x1000_0000); // MALLOC_CAP_SPIRAM

        println!("Free heap: {} bytes", free_heap);
        println!("Min free heap: {} bytes", min_heap);
        println!("Free internal heap: {} bytes", free_internal);
        println!("Free PSRAM: {} bytes", free_psram);
    }
}
