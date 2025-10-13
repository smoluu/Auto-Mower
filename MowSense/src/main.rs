use std::{ sync::{Arc, Mutex}, thread, time::Duration };
use esp_idf_hal::{ prelude::Peripherals, units::Hertz};
use esp_idf_svc::{eventloop::EspSystemEventLoop, hal::*, nvs::EspDefaultNvsPartition, wifi::EspWifi};
use esp_idf_hal::i2c::*;

use log::{ self, info, Log };
use esp_idf_sys::*;
use ota::init_ota;

use crate::{i2c::scan_i2c_bus, ota::start_ota_polling, wifi::wifi_connect};
mod ota;
mod wifi;
mod i2c;
static PIN_UART0_TX: i32 = 43;
static PIN_UART0_RX: i32 = 44;
static PIN_LED_RGB: i32 = 48;
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
    let mut wifi = EspWifi::new(peripherals.modem, sys_loop.clone(), nvs)?;
    wifi = wifi_connect(wifi).unwrap();

    let sda = peripherals.pins.gpio8;
    let scl = peripherals.pins.gpio9;

    // I2C configuration
    let config = I2cConfig::new().baudrate(Hertz(400_000)); // 400 kHz
    let mut i2c = I2cDriver::new(peripherals.i2c0, sda, scl, &config)?;
    // Scan the whole i2c bus for devices
    scan_i2c_bus(&mut i2c, 0x03, 0x77,10);

    // Check OTA partitions
    info!("Init ota: {:?}", init_ota()?);

    // Start OTA firmware update polling
    let ota = match start_ota_polling(OTA_SERVER_URL, FIRMWARE_VERSION, OTA_SERVER_POLLING_RATE) {
        Ok(_) => {},
        Err(e) => panic!("OTA polling start failed => {}", e)
    };

    loop {
        print_memory_info();
        if wifi.is_connected().map_err(|e| anyhow::anyhow!("Connection check failed: {}", e))? {
            if let Ok(ip_info) = wifi.sta_netif().get_ip_info() {
                info!("Wi-Fi is active. IP: {}", ip_info.ip);
            }
        } else {
            info!("Wi-Fi disconnected, attempting reconnect...");
            wifi.connect();
            while !wifi.is_connected().map_err(|e| anyhow::anyhow!("Reconnect failed: {}", e))? {
                thread::sleep(Duration::from_millis(500));
            }
            let ip_info = wifi.sta_netif().get_ip_info()?;
            info!("Reconnected! IP: {}", ip_info.ip);
            // Turn RGB LED off (high for common anode)
            unsafe {
                //esp_idf_sys::gpio_set_level(48, 1);
            }
        }
        thread::sleep(Duration::from_secs(5)); // Blink interval
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
