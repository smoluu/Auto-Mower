use log::info;
use anyhow::{ Ok, Result };
use esp_idf_svc::wifi::{ AccessPointConfiguration, AuthMethod, BlockingWifi, Configuration, EspWifi };

pub fn wifi_ap_setup(
    mut wifi: BlockingWifi<EspWifi<'static>>
) -> Result<BlockingWifi<EspWifi<'static>>> {
    let ssid = env!("WIFI_SSID");
    let password = env!("WIFI_PASSWORD");

    // Wi-Fi configuration
    let ap_config = Configuration::AccessPoint(AccessPointConfiguration {
        ssid: ssid.parse().unwrap(),
        password: password.parse().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        channel: 11,
        ssid_hidden: false,
        ..Default::default()
    });

    // Apply configuration
    wifi.set_configuration(&ap_config)?;
    info!("Calling start()...");
    wifi.start()?;
    info!("start() finished → WiFi should be starting");

    info!("Waiting for netif up...");
    wifi.wait_netif_up()?;
    info!("netif is up!");
    
    let ip_info = wifi.wifi().ap_netif().get_ip_info()?;

    info!(
        "Wi-Fi AP started: \nIP: {} , Subnet: {} , DNS: {:?} , SDNS {:?}",
        ip_info.ip,
        ip_info.subnet,
        ip_info.dns,
        ip_info.secondary_dns
    );

    Ok(wifi)
}
