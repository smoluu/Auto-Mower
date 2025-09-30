use log::info;
use std::thread;
use std::time::Duration;
use anyhow::{Ok, Result};
use esp_idf_svc::wifi::{ EspWifi, AuthMethod, ClientConfiguration, Configuration };
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::hal::prelude::Peripherals;
use std::sync::mpsc::{ channel,  };

pub fn wifi_connect(mut wifi: EspWifi<'static>) -> Result<EspWifi<'static>, anyhow::Error> {

    let ssid = env!("WIFI_SSID");
    let password = env!("WIFI_PASSWORD");

    // Wi-Fi configuration
    let config = Configuration::Client(ClientConfiguration {
        ssid: ssid.parse().unwrap(),
        password: password.parse().unwrap(), // Replace with your Wi-Fi password
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    });

    // Channels for IP info
    let (ip_sender, ip_receiver) = channel();


    // Apply configuration
    wifi.set_configuration(&config)?;
    wifi.start()?;
    info!("Wi-Fi started, connecting...");

    // Wait for connection
    while !wifi.is_started().map_err(|e| anyhow::anyhow!("Wi-Fi start failed: {}", e))? {
        thread::sleep(Duration::from_millis(500));
    }

    wifi.connect()?;
    info!("Waiting for IP address...");

    while !wifi.is_connected().map_err(|e| anyhow::anyhow!("Connection failed: {}", e))? {
        thread::sleep(Duration::from_millis(500));
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;
    ip_sender.send(ip_info)?;
    info!("Connected to Wi-Fi! IP: {}", ip_info.ip);

    Ok(wifi)
}
