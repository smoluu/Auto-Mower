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
mod mpu;
mod bmp280;
static I2C_Timeout: u32 = 2000;
static PIN_UART0_TX: i32 = 43;
static PIN_UART0_RX: i32 = 44;
static PIN_LED_RGB: i32 = 48;

const PI: f32 = 3.141592;

// Sensor i2c adresses
static SENSOR_ADDR_GY273: u8 = 0x2c;
static SENSOR_ADDR_MPU: u8 = 0x68;
static SENSOR_ADDR_BMP280: u8 = 0x77;

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

    // MPU configuration
    let mut mpu = mpu::MPU::new(SENSOR_ADDR_MPU);
    mpu.configure(&mut i2c).map_err(|e| anyhow::anyhow!("Error configuring mpu sensor: {}", e))?;
    thread::sleep(Duration::from_millis(10));

    let mut bmp280 = bmp280::Bmp280::new(SENSOR_ADDR_BMP280);
    bmp280
        .configure(&mut i2c)
        .map_err(|e| anyhow::anyhow!("Error configuring bmp280 sensor: {}", e))?;
    thread::sleep(Duration::from_millis(10));

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

        // reading MPU
        let mpu_reading: mpu::MPUReading = mpu.read(&mut i2c);
        info!(
            "Roll: {:.2} Pitch: {:.2} Temp: {:.2}°C, Acc: {:.2}G",
            mpu_reading.roll,
            mpu_reading.pitch,
            mpu_reading.temperature_c,
            mpu_reading.acc_total
        );

        // reading bmp280
        let bmp_reading: bmp280::Bmp280Reading = bmp280.read(&mut i2c);
        info!(
            "BMP280 -> Pressure: {}Pa Temperature: {}°C",
            bmp_reading.pressure,
            bmp_reading.temperature
        );

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
