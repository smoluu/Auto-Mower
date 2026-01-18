use std::{
    net::{ SocketAddr, UdpSocket },
    sync::{ Arc, Mutex },
    thread,
    time::{ Duration, Instant },
};
use esp_idf_hal::{
    can::AsyncCanDriver,
    gpio::{ self, AnyIOPin, AnyOutputPin, Output, PinDriver, Pins },
    io::Read,
    ledc::{ LedcDriver, LedcTimer, LedcTimerDriver, config::TimerConfig },
    prelude::Peripherals,
    timer::TimerDriver,
    uart,
    units::Hertz,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    timer::EspTimerService,
    wifi::{ AsyncWifi, BlockingWifi, EspWifi },
};
use esp_idf_hal::i2c::*;

use log::{ self, Log, debug, error, info, warn };
use esp_idf_sys::*;
use ota::init_ota;

use crate::packets::Packet;

mod ota;
mod wifi;
mod i2c;
mod gy273;
mod mpu;
mod bmp280;
mod lidar;
mod buzzer;
mod drive;
mod mower;
mod packets;

static PIN_LED_RGB: i32 = 48;

const PI: f32 = 3.141592;

const PNTT_PACKET_INTERVAL: Duration = Duration::from_millis(50);

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
    let mut timer_service = EspTimerService::new().unwrap();
    let nvs = EspDefaultNvsPartition::take().ok();
    let wifi_driver = EspWifi::new(peripherals.modem, sys_loop.clone(), nvs)?;
    let wifi_blocking = BlockingWifi::wrap(wifi_driver, sys_loop)?;
    let wifi = wifi::wifi_ap_setup(wifi_blocking)?;

    let socket = UdpSocket::bind("0.0.0.0:6968")?;
    socket.set_nonblocking(true)?;
    let mut last_heartbeat = Instant::now();
    let heartbeat_interval = Duration::from_millis(80); // 80 ms → ~12.5 Hz, perfect

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

    // L298N Wheel Driver setup
    let motor_left_in1 = PinDriver::output(peripherals.pins.gpio11)?;
    let motor_left_in2 = PinDriver::output(peripherals.pins.gpio12)?;
    let motor_right_in1 = PinDriver::output(peripherals.pins.gpio36)?;
    let motor_right_in2 = PinDriver::output(peripherals.pins.gpio37)?;

    // PWM setup for motor driver speed control
    let md_timer_config = TimerConfig::new()
        .frequency(Hertz(20_000))
        .resolution(esp_idf_hal::ledc::Resolution::Bits10);
    let timer = LedcTimerDriver::new(peripherals.ledc.timer0, &md_timer_config)?;
    let motor_left_pwm = LedcDriver::new(
        peripherals.ledc.channel0,
        &timer,
        peripherals.pins.gpio10
    )?;
    let motor_right_pwm = LedcDriver::new(
        peripherals.ledc.channel1,
        &timer,
        peripherals.pins.gpio38
    )?;

    let mut drive = drive::Drive::new(
        motor_left_pwm,
        motor_right_pwm,
        motor_left_in1,
        motor_left_in2,
        motor_right_in1,
        motor_right_in2,
        0.1
    );

    // // Setup BTS7960 / Mower blade motor
    // let motor_l_en = PinDriver::output(peripherals.pins.gpio1)?;
    // let motor_r_en = PinDriver::output(peripherals.pins.gpio2)?;
    // let mower_timer_config = TimerConfig::new()
    //     .frequency(Hertz(20_000))
    //     .resolution(esp_idf_hal::ledc::Resolution::Bits10);
    // let mower_timer = LedcTimerDriver::new(peripherals.ledc.timer2, &mower_timer_config)?;
    // let mower_pwm = LedcDriver::new(peripherals.ledc.channel3, &mower_timer, peripherals.pins.gpio4)?;
    // let mut mower = mower::Mower::new(mower_pwm, motor_l_en, motor_r_en, 0.1);

    // mower.set_speed(0.7);
    // thread::sleep(Duration::from_millis(2000));
    // mower.set_speed(1.0);
    // thread::sleep(Duration::from_millis(2000));
    // mower.set_speed(0.0);

    // Check OTA partitions
    //info!("Init ota: {:?}", init_ota()?);

    // Start OTA firmware update polling
    // let ota = match start_ota_polling(OTA_SERVER_URL, FIRMWARE_VERSION, OTA_SERVER_POLLING_RATE) {
    //     Ok(_) => {}
    //     Err(e) => panic!("OTA polling start failed => {}", e),
    // };

    let mut mowmaster_src: Option<SocketAddr> = None;
    let mut last_pntt_instant: Instant = Instant::now();
    loop {
        //print_memory_info();

        // Time stamp for PNTT packets
        let timestamp_us = unsafe { esp_timer_get_time() };

        // Reading GY273
        let gy273_reading = gy273.read(&mut i2c);
        //info!("Heading: {:.2}", gy273_reading.heading);

        // Reading MPU
        let mpu_reading: mpu::MPUReading = mpu.read(&mut i2c);
        // info!(
        // "Roll: {:.2} Pitch: {:.2} Temp: {:.2}°C, Acc: {:.2}G",
        // mpu_reading.roll,
        // mpu_reading.pitch,
        // mpu_reading.temperature_c,
        // mpu_reading.acc_total
        // );

        // Reading bmp280
        let bmp_reading: bmp280::Bmp280Reading = bmp280.read(&mut i2c);
        // info!(
        // "BMP280 -> Pressure: {}Pa Temperature: {}°C",
        // bmp_reading.pressure,
        // bmp_reading.temperature
        // );

        // Reading lidar
        //drive.set_speed(1.0, 1.0);

        let mut buf = [0; 2048];
        // Receive UDP packets from MowMaster
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                let payload = &buf[..len];
                info!("Received {} bytes from {}: {:?}", len, src, payload);
                mowmaster_src = Some(src);

                if let Some(packet) = Packet::parse(payload) {
                    match packet {
                        Packet::Ctrl { throttle, steering, mode } => {
                            info!("CTRL PACKET: {:?}", &payload);
                            info!("THROTTLE {}", throttle);
                            let (left_target, right_target) = drive::arcade_to_diff(
                                throttle,
                                steering
                            );
                            drive.set_speed(left_target, right_target);
                        }
                        Packet::Keepalive {} => {
                            // Echo keepalive
                            info!("KEEPALIVE received - echoing back");
                            let _ = socket.send_to(b"KEEPALIVE", src);
                        }
                        Packet::Unknown { raw } => {
                            info!("Unknown packet: {:02x?}", raw);
                        }
                        _ => {
                            info!("Unknown packet");
                        }
                    }
                }
            }
            Err(e) =>
                match e.kind() {
                    std::io::ErrorKind::WouldBlock => {} //  No data, skip
                    _ => {
                        info!("encountered IO error: {e}");
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
        }

        // Send UDP packets to MowMaster if destination src is set

        if let Some(src) = mowmaster_src {
            // Create PNTT packet

            if last_pntt_instant.elapsed() >= PNTT_PACKET_INTERVAL {
                let packet = Packet::PNTT {
                    heading: gy273_reading.heading,
                    roll: mpu_reading.roll,
                    pitch: mpu_reading.pitch,
                    temp_c_0: mpu_reading.temperature_c,
                    acc_total: mpu_reading.acc_total,
                    pressure: bmp_reading.pressure,
                    temp_c_1: bmp_reading.temperature,
                    timestamp_us: timestamp_us,
                };
                if let Some(bytes) = packet.to_bytes() {
                    // Send PNTT packet
                    if let Err(e) = socket.send_to(&bytes, src) {
                        warn!("Send failed: {} - dropping packet", e);
                    }
                    last_pntt_instant = Instant::now();
                }
            }
        }
    }
}
