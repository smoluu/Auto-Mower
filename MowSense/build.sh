#!/bin/bash

#
#   Builds project and generates a ESP-IDF application image 
#

# export IDF_PATH=~/esp-idf
# . "$IDF_PATH/export.sh"

echo "Building Rust project..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "Error: Cargo build failed"
    exit 1
fi

echo "Generating OTA binary..."
./generate_ota.sh
if [ $? -ne 0 ]; then
    echo "Error: OTA binary generation failed"
    exit 1
fi

echo "Build and OTA binary generation complete!"