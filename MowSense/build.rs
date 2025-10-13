use std::path::Path;
use std::fs;

fn main() {
    embuild::espidf::sysenv::output();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("partitions.csv");
    fs::copy("partitions.csv", dest_path).expect("Failed to copy partitions.csv");
    println!("cargo:rerun-if-changed=partitions.csv");
}