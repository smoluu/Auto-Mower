use std::{ sync::Arc, thread, time::Duration };
use esp_idf_svc::ota::EspOta;
use ureq;
use std::io::Read;
use log::{ info, error };
use esp_idf_sys::EspError;
use base64::{ engine::general_purpose, Engine as _ };

#[derive(Clone)]
struct OtaConfig {
    firmware_version: String,
    firmware_binary_url: String,
    firware_version_url: String,
    interval_secs: u64,
    auth: String,
}

pub fn start_ota_polling(
    ota_server_url: &str,
    firware_version: &str,
    interval_secs: u64
) -> Result<(), EspError> {
    // Create auth header string (if using nginx proxy with access list)
    let auth_string = format!("{}:{}", env!("PROXY_USER"), env!("PROXY_PASSWORD"));
    let auth = general_purpose::STANDARD.encode(auth_string.as_bytes());

    // Create configuration and wrap in Arc for thread safety
    let config = Arc::new(OtaConfig {
        firmware_version: firware_version.to_string(),
        firmware_binary_url: ota_server_url.to_string() + "/firmware.bin",
        firware_version_url: ota_server_url.to_string() + "/version.txt",
        interval_secs,
        auth,
    });

    // Spawn the OTA polling thread with a larger stack size
    let config_clone = Arc::clone(&config);
    let builder = thread::Builder
        ::new()
        .name("ota_poller".into())
        .stack_size(32 * 1024); // 32KB
    builder
        .spawn(move || {
            info!("Starting OTA polling thread for URL: {}", config_clone.firmware_binary_url);
            loop {
                match check_and_update(&config_clone) {
                    Ok(should_reboot) => {
                        info!("OTA check completed. Reboot required: {}", should_reboot);
                        if should_reboot {
                            info!("Initiating reboot after successful OTA update");
                            unsafe {
                                esp_idf_sys::esp_restart();
                            }
                        }
                    }
                    Err(e) => {
                        error!("OTA check failed: {:?}", e);
                    }
                }

                thread::sleep(Duration::from_secs(config_clone.interval_secs));
            }
        })
        .unwrap();

    Ok(())
}

fn check_and_update(config: &OtaConfig) -> Result<bool, Box<dyn std::error::Error>> {
    // Check for firware version change from firmware_binary_url/version.txt
    info!("Checking for firmware version at {}", config.firware_version_url);
    let response = ureq
        ::get(&config.firware_version_url)
        .set("Authorization", &format!("Basic {}", &config.auth)) // optional auth header if using nginx proxy auth
        .timeout(Duration::from_secs(5))
        .call()
        .map_err(|e| format!("Failed to fetch firmware version: {}", e))?;
    // Check HTTP status
    if response.status() != 200 {
        return Err(format!("Server returned status {}", response.status()).into());
    }
    let latest_version_str = response
        .into_string()
        .map_err(|e| format!("Failed to parse version.txt => {}", e))?
        .trim()
        .to_string();

    info!("Latest firwmware version fetched => {}", latest_version_str);

    if config.firmware_version == latest_version_str {
        info!("Firmware version matches latest version");
        return Ok(false);
    }
    info!("New firmware version detected");

    info!("Checking for firmware update at {}", config.firmware_binary_url);
    // Perform HTTP request
    info!("Starting HTTP request");
    let response = ureq
        ::get(&config.firmware_binary_url)
        .set("Authorization", &format!("Basic {}", &config.auth)) // optional auth header if using nginx proxy auth
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Failed to fetch firmware: {}", e))?;
    info!("HTTP request completed, status: {}", response.status());

    // Check HTTP status
    if response.status() != 200 {
        return Err(format!("Server returned status {}", response.status()).into());
    }
    return Ok(false);
    // Initialize OTA
    let mut ota = EspOta::new().map_err(|e| format!("Failed to initialize OTA: {}", e))?;
    let mut update = ota
        .initiate_update()
        .map_err(|e| format!("Failed to initiate OTA update: {}", e))?;

    // Stream firmware to OTA partition to reduce memory usage
    let mut reader = response.into_reader();
    let mut buffer = [0u8; 4096]; // 4KB chunks
    let mut total_bytes = 0;
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read firmware: {}", e))?;
        if bytes_read == 0 {
            break;
        }
        update
            .write(&buffer[..bytes_read])
            .map_err(|e| format!("Failed to write firmware: {}", e))?;
        total_bytes += bytes_read;
    }

    if total_bytes == 0 {
        info!("No new firmware available.");
        return Ok(false);
    }

    info!("Firmware downloaded: {} bytes", total_bytes);

    // Optional: Validate firmware (implement as needed)
    // if !is_valid_firmware(&buffer, total_bytes) {
    //     return Err("Invalid firmware".into());
    // }

    // Complete the OTA update
    update.complete().map_err(|e| format!("Failed to complete OTA update: {}", e))?;
    info!("OTA update completed successfully.");

    Ok(true)
}
