use std::{ env, fs, path::PathBuf, process::Command };
fn main() {
    embuild::espidf::sysenv::output();

    // Directory to store the OTA binary
    let output_dir = "ota-binary";
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // The built ELF file
    let target_elf = std::fs::canonicalize(
        "target/xtensa-esp32s3-espidf/release/turf_terminator_5000"
    ).expect("Failed to find ELF file");

    // Run esptool.py to generate .bin
    let status = Command::new("esptool.py")
        .arg("--chip")
        .arg("esp32s3")
        .arg("elf2image")
        .arg(&target_elf)
        .status()
        .expect("Failed to run esptool.py");

    if !status.success() {
        panic!("esptool.py elf2image failed with exit code: {:?}", status.code());
    }

    // Generated .bin path (esptool.py naming convention)
    let generated_bin = target_elf
        .parent()
        .unwrap()
        .join("turf_terminator_5000.bin");

    if !generated_bin.exists() {
        panic!("Generated .bin not found at expected path: {:?}", generated_bin);
    }

    // Destination path in ota-binary folder
    let output_bin_path = PathBuf::from(output_dir).join("firmware.bin");

    // Copy to ota-binary/firmware.bin
    fs::copy(&generated_bin, &output_bin_path)
        .expect("Failed to copy firmware binary");

    println!("OTA binary created at {:?}", output_bin_path);
}