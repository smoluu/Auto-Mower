use std::{ sync::{ Arc, Mutex }, thread, time::Duration };
use esp_idf_hal::{
    gpio::{ self, AnyIOPin, Output, PinDriver, Pins },
    io::Read,
    ledc::{ config::TimerConfig, LedcDriver, LedcTimer, LedcTimerDriver },
    prelude::Peripherals,
    timer::TimerDriver,
    uart,
    units::Hertz,
};
use esp_idf_svc::{ eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition, wifi::EspWifi };
use esp_idf_hal::i2c::*;

use log::{ self, debug, info, Log };
use esp_idf_sys::*;
use ota::init_ota;

mod ota;
mod wifi;
mod i2c;
mod gy273;
mod mpu;
mod bmp280;
mod lidar;
mod buzzer;

static I2C_Timeout: u32 = 2000;
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

    let peripherals = Peripherals::take().unwrap();

    //  Wifi setup
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().ok();
    //let mut wifi = EspWifi::new(peripherals.modem, sys_loop.clone(), nvs)?;
    //wifi = wifi_connect(wifi).unwrap();

    // UART driver setup
    // https://docs.esp-rs.org/esp-idf-hal/esp_idf_hal/uart/index.html
    let uart_rx = peripherals.pins.gpio18;
    let uart_config = uart::config::Config
        ::default()
        .baudrate(Hertz(230400))
        .data_bits(uart::config::DataBits::DataBits8)
        .stop_bits(uart::config::StopBits::STOP1)
        .parity_none()
        .flow_control(uart::config::FlowControl::None)
        .rx_fifo_size(4096); // RX buffer size
    let mut uart = uart::UartRxDriver
        ::new(
            peripherals.uart1,
            uart_rx,
            Option::<AnyIOPin>::None,
            Option::<AnyIOPin>::None,
            &uart_config
        )
        .map_err(|e| anyhow::anyhow!("Error configuring uart driver: {e}"))?;

    // I2C configuration
    let sda = peripherals.pins.gpio8;
    let scl = peripherals.pins.gpio9;

    let i2c_config = I2cConfig::new()
        .baudrate(Hertz(100_000)) // 100 kHz
        .sda_enable_pullup(true)
        .scl_enable_pullup(true);
    let mut i2c = I2cDriver::new(peripherals.i2c0, sda, scl, &i2c_config)?;

    i2c::scan_i2c_bus(&mut i2c);

    // GY-273 Configuration

    let mut gy273 = gy273::GY273::new(SENSOR_ADDR_GY273);
    gy273
        .configure(&mut i2c)
        .map_err(|e| anyhow::anyhow!("Error configuring gy273 sensor: {}", e))?;

    // MPU configuration
    let mut mpu = mpu::MPU::new(SENSOR_ADDR_MPU);
    mpu.configure(&mut i2c).map_err(|e| anyhow::anyhow!("Error configuring mpu sensor: {}", e))?;

    let mut bmp280 = bmp280::Bmp280::new(SENSOR_ADDR_BMP280);
    bmp280
        .configure(&mut i2c)
        .map_err(|e| anyhow::anyhow!("Error configuring bmp280 sensor: {}", e))?;

    // Beeper setup
    let buzzer_timer_cfg = TimerConfig::new()
        .frequency(Hertz(20_000))
        .resolution(esp_idf_hal::ledc::Resolution::Bits10);
    let mut buzzer_timer = LedcTimerDriver::new(
        peripherals.ledc.timer1,
        &buzzer_timer_cfg
    ).unwrap();
    let mut buzzer_pwm = LedcDriver::new(
        peripherals.ledc.channel2,
        &buzzer_timer,
        peripherals.pins.gpio5
    ).unwrap();

    // Controlling buzzer

    // buzzer_pwm.set_duty(512)?;
    // buzzer_timer.set_frequency(Hertz(800))?;
    // thread::sleep(Duration::from_millis(1000));
    // buzzer_pwm.set_duty(0)?;
    
    thread::spawn(move || {
        buzzer::play(&mut buzzer_pwm, &mut buzzer_timer, buzzer::STARTUP);
    });

    // L298N setup
    let mut md_l_in1 = PinDriver::output(peripherals.pins.gpio11)?;
    let mut md_l_in2 = PinDriver::output(peripherals.pins.gpio12)?;
    let mut md_r_in1 = PinDriver::output(peripherals.pins.gpio36)?;
    let mut md_r_in2 = PinDriver::output(peripherals.pins.gpio37)?;

    // PWM setup for motor driver speed control
    let md_timer_config = TimerConfig::new()
        .frequency(Hertz(20_000))
        .resolution(esp_idf_hal::ledc::Resolution::Bits10);
    let timer= LedcTimerDriver::new(peripherals.ledc.timer0, &md_timer_config)?;
    let mut md_l_pwm = LedcDriver::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio10)?;
    let mut md_r_pwm = LedcDriver::new(peripherals.ledc.channel1, &timer, peripherals.pins.gpio38)?;

    // Controlling motor drivers PWM duty 0-1023 on 10Bit
    md_l_in1.set_high()?;
    md_l_in2.set_low()?;
    md_r_in1.set_high()?;
    md_r_in2.set_low()?;
    md_l_pwm.set_duty(600)?;
    md_r_pwm.set_duty(600)?;
    // reverse
    // md_l_in1.set_low()?;
    // md_l_in2.set_high()?;
    // md_r_in1.set_low()?;
    // md_r_in2.set_high()?;

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

        // Reading GY273
        let gy273_reading = gy273.read(&mut i2c);
        info!("Heading: {:.2}", gy273_reading.heading);

        // Reading MPU
        let mpu_reading: mpu::MPUReading = mpu.read(&mut i2c);
        info!(
            "Roll: {:.2} Pitch: {:.2} Temp: {:.2}°C, Acc: {:.2}G",
            mpu_reading.roll,
            mpu_reading.pitch,
            mpu_reading.temperature_c,
            mpu_reading.acc_total
        );

        // Reading bmp280
        let bmp_reading: bmp280::Bmp280Reading = bmp280.read(&mut i2c);
        info!(
            "BMP280 -> Pressure: {}Pa Temperature: {}°C",
            bmp_reading.pressure,
            bmp_reading.temperature
        );

        // Reading lidar

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
